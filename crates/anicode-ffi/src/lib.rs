//! The C functions the MLT module calls. See mlt-module/anicode.h.
//!
//! Every function catches panics, so nothing unwinds into MLT or Shotcut.
//! A handle must not be used from two threads at once; the module holds a
//! lock around each call.

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_double, c_int};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anicode_lua::{Config, Engine, Settings, params_path};
use anicode_render::{CpuRenderer, ImageAlphaType};

pub struct AnicodeEngine {
    scene: PathBuf,
    scene_modified: Option<SystemTime>,
    params_modified: Option<SystemTime>,
    /// The Shotcut project whose markers time the scene, if one was found.
    project: Option<PathBuf>,
    project_modified: Option<SystemTime>,
    /// The last scene that loaded without errors. Kept when an edit breaks
    /// the script, so the clip keeps showing something.
    loaded: Option<(Engine, Config)>,
    renderer: Option<CpuRenderer>,
    last_error: CString,
}

impl AnicodeEngine {
    /// Reloads when the scene file, its params file or the Shotcut project
    /// changes. Saving the project after moving a marker therefore re-times
    /// the scene.
    fn reload_if_changed(&mut self) -> c_int {
        let scene_modified = modified_time(&self.scene);
        let scene_changed = scene_modified.is_none() || scene_modified != self.scene_modified;
        let params_modified = modified_time(&params_path(&self.scene));
        let project_modified = self.project.as_deref().and_then(modified_time);
        if !scene_changed
            && params_modified == self.params_modified
            && project_modified == self.project_modified
        {
            return 0;
        }
        self.scene_modified = scene_modified;
        self.params_modified = params_modified;
        match self.load(scene_changed) {
            Ok(loaded) => {
                self.loaded = Some(loaded);
                self.set_error("");
                1
            }
            Err(error) => {
                self.set_error(&error);
                -1
            }
        }
    }

    fn load(&mut self, scene_changed: bool) -> Result<(Engine, Config), String> {
        let config = Config::find_for(&self.scene)?;
        // Searching for the project walks the folders, so only do it when
        // the scene itself changes (or opens), or the project went away.
        let project_gone = self
            .project
            .as_deref()
            .is_some_and(|project| !project.is_file());
        if scene_changed || project_gone {
            self.project = anicode_shotcut::find_project(&config.root, &self.scene);
        }
        self.project_modified = self.project.as_deref().and_then(modified_time);
        let markers = match &self.project {
            Some(project) => anicode_shotcut::clip_markers(project, &self.scene)?,
            None => HashMap::new(),
        };
        let settings = Settings {
            frame_height: config.render.frame_height,
            default_block_seconds: config.timing.default_block_seconds,
            default_animation_seconds: config.timing.default_animation_seconds,
        };
        let engine = Engine::load(&self.scene, &markers, settings)?;
        Ok((engine, config))
    }

    /// Returns -1 when a new error appears, so the caller logs it once
    /// rather than on every frame.
    fn render(
        &mut self,
        time: f64,
        width: u16,
        height: u16,
        rgba: &mut [u8],
        stride: usize,
    ) -> c_int {
        let mut status = 0;
        if let Err(error) = self.draw(time, width, height, rgba, stride) {
            rgba.fill(0);
            if self.set_error(&error) {
                status = -1;
            }
        }
        if !self.last_error.is_empty() {
            draw_error_bar(rgba, width, height, stride);
        }
        status
    }

    fn draw(
        &mut self,
        time: f64,
        width: u16,
        height: u16,
        rgba: &mut [u8],
        stride: usize,
    ) -> Result<(), String> {
        let Some((engine, config)) = &self.loaded else {
            rgba.fill(0);
            return Ok(());
        };
        let items = engine.display_list(time, width.into(), height.into())?;

        let renderer = match &mut self.renderer {
            Some(renderer) if (renderer.width(), renderer.height()) == (width, height) => renderer,
            _ => self.renderer.insert(CpuRenderer::new(width, height)),
        };
        // MLT expects straight (not premultiplied) alpha.
        let pixels = renderer
            .render(&items, config.render.background)
            .take_rgba8(ImageAlphaType::Alpha);

        let row_bytes = usize::from(width) * 4;
        for (row, target) in pixels.chunks(row_bytes).zip(rgba.chunks_mut(stride)) {
            target[..row_bytes].copy_from_slice(row);
        }
        Ok(())
    }

    /// Returns whether the error text changed.
    fn set_error(&mut self, error: &str) -> bool {
        let error = CString::new(error.replace('\0', " ")).unwrap_or_default();
        let changed = error != self.last_error;
        self.last_error = error;
        changed
    }
}

/// A red bar across the top of the frame while the scene has an error. The
/// message itself goes to the log until the engine can draw text.
fn draw_error_bar(rgba: &mut [u8], width: u16, height: u16, stride: usize) {
    const RED: [u8; 4] = [220, 40, 40, 255];
    let bar_rows = (usize::from(height) / 30).max(4);
    for row in rgba.chunks_mut(stride).take(bar_rows) {
        for pixel in row[..usize::from(width) * 4].chunks_mut(4) {
            pixel.copy_from_slice(&RED);
        }
    }
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

fn guard<T>(fallback: T, body: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(fallback)
}

/// Opens a scene file. Returns NULL only if `scene_path` is not valid
/// UTF-8. If the script fails, the handle is still returned and
/// `anicode_last_error` says why.
///
/// # Safety
/// `scene_path` must be a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn anicode_open(scene_path: *const c_char) -> *mut AnicodeEngine {
    guard(std::ptr::null_mut(), || {
        // SAFETY: the caller passes a valid C string.
        let Ok(path) = unsafe { CStr::from_ptr(scene_path) }.to_str() else {
            return std::ptr::null_mut();
        };
        let mut engine = AnicodeEngine {
            scene: PathBuf::from(path),
            scene_modified: None,
            params_modified: None,
            project: None,
            project_modified: None,
            loaded: None,
            renderer: None,
            last_error: CString::default(),
        };
        engine.reload_if_changed();
        Box::into_raw(Box::new(engine))
    })
}

/// Reloads the scene if its file changed. Returns 1 if reloaded, 0 if
/// unchanged, -1 if the new script has an error (the old scene stays).
///
/// # Safety
/// `engine` must come from `anicode_open` and not be closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn anicode_reload_if_changed(engine: *mut AnicodeEngine) -> c_int {
    // SAFETY: the caller passes a live handle.
    guard(-1, || unsafe { &mut *engine }.reload_if_changed())
}

/// Scene length in seconds, or 0 if no scene has loaded yet.
///
/// # Safety
/// `engine` must come from `anicode_open` and not be closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn anicode_duration(engine: *mut AnicodeEngine) -> c_double {
    // SAFETY: the caller passes a live handle.
    guard(0.0, || {
        unsafe { &*engine }
            .loaded
            .as_ref()
            .map_or(0.0, |(engine, _)| engine.duration())
    })
}

/// Draws the frame at `time` seconds into `rgba` (straight alpha). Returns
/// 0 on success, -1 on error (the buffer is then cleared).
///
/// # Safety
/// `engine` must be a live handle and `rgba` must hold `height` rows of
/// `stride` bytes, with `stride >= width * 4`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn anicode_render(
    engine: *mut AnicodeEngine,
    time: c_double,
    width: c_int,
    height: c_int,
    rgba: *mut u8,
    stride: c_int,
) -> c_int {
    guard(-1, || {
        let (Ok(width), Ok(height), Ok(stride)) = (
            u16::try_from(width),
            u16::try_from(height),
            usize::try_from(stride),
        ) else {
            return -1;
        };
        if stride < usize::from(width) * 4 {
            return -1;
        }
        // SAFETY: the caller guarantees the buffer size and a live handle.
        let rgba = unsafe { std::slice::from_raw_parts_mut(rgba, stride * usize::from(height)) };
        unsafe { &mut *engine }.render(time, width, height, rgba, stride)
    })
}

/// The last error as `file:line: message`, or an empty string. Valid until
/// the next call on this handle.
///
/// # Safety
/// `engine` must come from `anicode_open` and not be closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn anicode_last_error(engine: *mut AnicodeEngine) -> *const c_char {
    // SAFETY: the caller passes a live handle.
    guard(c"internal error".as_ptr(), || {
        unsafe { &*engine }.last_error.as_ptr()
    })
}

/// Frees the handle.
///
/// # Safety
/// `engine` must come from `anicode_open`, and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn anicode_close(engine: *mut AnicodeEngine) {
    if engine.is_null() {
        return;
    }
    // SAFETY: the handle came from Box::into_raw in anicode_open.
    guard((), || drop(unsafe { Box::from_raw(engine) }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(path: &str) -> *mut AnicodeEngine {
        let path = CString::new(path).unwrap();
        unsafe { anicode_open(path.as_ptr()) }
    }

    fn error(engine: *mut AnicodeEngine) -> String {
        unsafe { CStr::from_ptr(anicode_last_error(engine)) }
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn renders_example_scene() {
        let engine = open(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/basics.lua"
        ));
        assert_eq!(error(engine), "");
        assert_eq!(unsafe { anicode_duration(engine) }, 8.0);

        let (width, height) = (160, 90);
        let mut rgba = vec![0u8; width * height * 4];
        let status = unsafe { anicode_render(engine, 5.0, 160, 90, rgba.as_mut_ptr(), 160 * 4) };
        assert_eq!(status, 0, "{}", error(engine));
        assert!(rgba.chunks(4).any(|pixel| pixel[3] == 255));
        unsafe { anicode_close(engine) };
    }

    #[test]
    fn shotcut_markers_time_the_scene_and_reload_on_save() {
        let root = std::env::temp_dir().join(format!("anicode-markers-{}", std::process::id()));
        std::fs::create_dir_all(root.join("config")).unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::copy(
            repo.join("config/anicode.toml"),
            root.join("config/anicode.toml"),
        )
        .unwrap();
        std::fs::write(
            root.join("shape.lua"),
            "local c = Circle {}\nat \"intro\" { fade_in(c) }\nat \"move\" { c:shift(UP) }\n",
        )
        .unwrap();
        let project = root.join("edit.mlt");
        let xml = include_str!("../../anicode-shotcut/tests/fixtures/shotcut-26.9.mlt");
        std::fs::write(&project, xml).unwrap();

        let engine = open(root.join("shape.lua").to_str().unwrap());
        // "move" is at 3.96 s on the timeline and the clip starts at 1.08 s.
        let duration = unsafe { anicode_duration(engine) };
        assert!(
            (duration - (2.88 + 2.0)).abs() < 1e-9,
            "{duration} {}",
            error(engine)
        );

        // Move the marker to 5.08 s and save; the next frame picks it up.
        std::fs::write(
            &project,
            xml.replace("00:00:03.960</property>", "00:00:05.080</property>"),
        )
        .unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&project)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_eq!(unsafe { anicode_reload_if_changed(engine) }, 1);
        let duration = unsafe { anicode_duration(engine) };
        assert!((duration - (4.0 + 2.0)).abs() < 1e-9, "{duration}");

        unsafe { anicode_close(engine) };
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn reloads_when_the_params_file_changes() {
        let root = std::env::temp_dir().join(format!("anicode-params-{}", std::process::id()));
        std::fs::create_dir_all(root.join("config")).unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::copy(
            repo.join("config/anicode.toml"),
            root.join("config/anicode.toml"),
        )
        .unwrap();
        std::fs::write(
            root.join("wait.lua"),
            "play { seconds = param(\"wait\", 2) }\n",
        )
        .unwrap();

        let engine = open(root.join("wait.lua").to_str().unwrap());
        assert_eq!(
            unsafe { anicode_duration(engine) },
            2.0,
            "{}",
            error(engine)
        );
        std::fs::write(root.join("wait.params.lua"), "return { wait = 3 }\n").unwrap();
        assert_eq!(unsafe { anicode_reload_if_changed(engine) }, 1);
        assert_eq!(unsafe { anicode_duration(engine) }, 3.0);

        unsafe { anicode_close(engine) };
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn script_errors_are_reported_not_raised() {
        let engine = open("/no/such/scene.lua");
        assert!(error(engine).contains("scene.lua"), "{}", error(engine));
        let mut rgba = vec![1u8; 8 * 8 * 4];
        unsafe { anicode_render(engine, 0.0, 8, 8, rgba.as_mut_ptr(), 32) };
        // An error bar on top, transparent below it.
        assert_eq!(&rgba[..4], &[220, 40, 40, 255]);
        assert!(rgba[8 * 4 * 4..].iter().all(|&byte| byte == 0));
        unsafe { anicode_close(engine) };
    }
}
