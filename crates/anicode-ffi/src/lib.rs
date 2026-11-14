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

use anicode_lua::{Config, Engine, Settings};
use anicode_render::{CpuRenderer, ImageAlphaType};

pub struct AnicodeEngine {
    scene: PathBuf,
    modified: Option<SystemTime>,
    /// The last scene that loaded without errors. Kept when an edit breaks
    /// the script, so the clip keeps showing something.
    loaded: Option<(Engine, Config)>,
    renderer: Option<CpuRenderer>,
    last_error: CString,
}

impl AnicodeEngine {
    fn reload_if_changed(&mut self) -> c_int {
        let modified = std::fs::metadata(&self.scene)
            .and_then(|metadata| metadata.modified())
            .ok();
        if modified.is_some() && modified == self.modified {
            return 0;
        }
        self.modified = modified;
        match load(&self.scene) {
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

    fn render(
        &mut self,
        time: f64,
        width: u16,
        height: u16,
        rgba: &mut [u8],
        stride: usize,
    ) -> c_int {
        let Some((engine, config)) = &self.loaded else {
            rgba.fill(0);
            return -1;
        };
        let items = match engine.display_list(time, width.into(), height.into()) {
            Ok(items) => items,
            Err(error) => {
                self.set_error(&error);
                rgba.fill(0);
                return -1;
            }
        };

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
        0
    }

    fn set_error(&mut self, error: &str) {
        self.last_error = CString::new(error.replace('\0', " ")).unwrap_or_default();
    }
}

fn load(scene: &Path) -> Result<(Engine, Config), String> {
    let config = Config::find_for(scene)?;
    let settings = Settings {
        frame_height: config.render.frame_height,
        default_block_seconds: config.timing.default_block_seconds,
    };
    let engine = Engine::load(scene, &HashMap::new(), settings)?;
    Ok((engine, config))
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
            modified: None,
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
    fn script_errors_are_reported_not_raised() {
        let engine = open("/no/such/scene.lua");
        assert!(error(engine).contains("scene.lua"), "{}", error(engine));
        let mut rgba = vec![1u8; 4 * 4 * 4];
        assert_eq!(
            unsafe { anicode_render(engine, 0.0, 4, 4, rgba.as_mut_ptr(), 16) },
            -1
        );
        assert!(rgba.iter().all(|&byte| byte == 0));
        unsafe { anicode_close(engine) };
    }
}
