//! A preview window for a scene: play it, scrub it, and see edits to the
//! scene and its params file as soon as they are saved. Dragging a shape
//! changes the params that place it and writes them to the params file.

mod drag;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use anicode_core::kurbo::{Point, Shape as _};
use anicode_core::{DrawItem, NodeId, hit_test};
use anicode_lua::{Config, Engine, ParamValue, load_scene, params_path, set_params};
use anicode_render::CpuRenderer;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, TextureHandle};
use serde::Deserialize;

use drag::{Drag, Mode};

const CONFIG_FILE: &str = "config/editor.toml";

#[derive(Deserialize)]
struct EditorConfig {
    preview: PreviewConfig,
    drag: DragConfig,
    watch: WatchConfig,
}

#[derive(Deserialize)]
struct DragConfig {
    step: f64,
    hit_slop: f64,
    min_gain: f64,
}

#[derive(Deserialize)]
struct PreviewConfig {
    scale: f64,
}

#[derive(Deserialize)]
struct WatchConfig {
    poll_seconds: f64,
}

fn main() -> eframe::Result {
    let Some(scene) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: anicode-editor scene.lua");
        std::process::exit(2);
    };
    let settings = match read_editor_config(&scene) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let title = format!("Anicode editor: {}", scene.display());
    eframe::run_native(
        &title,
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(Editor::new(scene, settings)))),
    )
}

fn read_editor_config(scene: &Path) -> Result<EditorConfig, String> {
    let path = Config::find_for(scene)?.root.join(CONFIG_FILE);
    let text =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

/// A scene that loaded without errors, with the config it was loaded with.
struct Loaded {
    engine: Engine,
    config: Config,
}

struct Editor {
    scene: PathBuf,
    settings: EditorConfig,
    /// The Shotcut project whose markers time the scene, if there is one.
    project: Option<PathBuf>,
    /// Kept when an edit breaks the script, so the preview keeps working.
    loaded: Option<Loaded>,
    error: Option<String>,
    /// When the scene, params and project files were last changed.
    file_times: Vec<Option<SystemTime>>,
    last_check: Instant,
    time: f64,
    playing: bool,
    last_tick: Instant,
    renderer: Option<CpuRenderer>,
    texture: Option<TextureHandle>,
    /// What the preview shows, for picking shapes under the cursor.
    items: Vec<DrawItem>,
    needs_render: bool,
    /// The node last clicked, outlined in the preview.
    selected: Option<NodeId>,
    drag: Option<Drag>,
    /// What the last click or drag did, or why it could not.
    message: String,
}

impl Editor {
    fn new(scene: PathBuf, settings: EditorConfig) -> Editor {
        let project = Config::find_for(&scene)
            .ok()
            .and_then(|config| anicode_shotcut::find_project(&config.root, &scene));
        let mut editor = Editor {
            scene,
            settings,
            project,
            loaded: None,
            error: None,
            file_times: vec![],
            last_check: Instant::now(),
            time: 0.0,
            playing: false,
            last_tick: Instant::now(),
            renderer: None,
            texture: None,
            items: vec![],
            needs_render: true,
            selected: None,
            drag: None,
            message: String::new(),
        };
        editor.reload();
        editor
    }

    fn watched_files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.scene.clone(), params_path(&self.scene)];
        files.extend(self.project.clone());
        files
    }

    fn reload(&mut self) {
        self.file_times = self
            .watched_files()
            .iter()
            .map(|path| modified_time(path))
            .collect();
        match load(&self.scene, self.project.as_deref()) {
            Ok(loaded) => {
                self.time = self.time.min(loaded.engine.duration());
                self.loaded = Some(loaded);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
        self.needs_render = true;
    }

    fn reload_if_changed(&mut self) {
        if self.last_check.elapsed().as_secs_f64() < self.settings.watch.poll_seconds {
            return;
        }
        self.last_check = Instant::now();
        let times: Vec<_> = self
            .watched_files()
            .iter()
            .map(|path| modified_time(path))
            .collect();
        if times != self.file_times {
            self.reload();
        }
    }

    fn duration(&self) -> f64 {
        self.loaded
            .as_ref()
            .map_or(0.0, |loaded| loaded.engine.duration())
    }

    fn advance_playhead(&mut self) {
        let elapsed = self.last_tick.elapsed().as_secs_f64();
        self.last_tick = Instant::now();
        if !self.playing {
            return;
        }
        self.time += elapsed;
        if self.time >= self.duration() {
            self.time = self.duration();
            self.playing = false;
        }
        self.needs_render = true;
    }

    fn render(&mut self, ctx: &egui::Context) {
        let Some(loaded) = &self.loaded else {
            return;
        };
        self.needs_render = false;
        let scale = self.settings.preview.scale;
        let width = (f64::from(loaded.config.render.width) * scale).round() as u16;
        let height = (f64::from(loaded.config.render.height) * scale).round() as u16;
        let items = match loaded
            .engine
            .display_list(self.time, width.into(), height.into())
        {
            Ok(items) => items,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let renderer = match &mut self.renderer {
            Some(renderer) if (renderer.width(), renderer.height()) == (width, height) => renderer,
            _ => self.renderer.insert(CpuRenderer::new(width, height)),
        };
        let pixmap = renderer.render(&items, loaded.config.render.background);
        let image = egui::ColorImage::from_rgba_premultiplied(
            [width.into(), height.into()],
            pixmap.data_as_u8_slice(),
        );
        match &mut self.texture {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => {
                self.texture =
                    Some(ctx.load_texture("preview", image, egui::TextureOptions::LINEAR));
            }
        }
        self.items = items;
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let label = if self.playing { "Pause" } else { "Play" };
            if ui.button(label).clicked() {
                self.toggle_play();
            }
            ui.label(format!("{:.2} / {:.2} s", self.time, self.duration()));
        });
        let blocks = self
            .loaded
            .as_ref()
            .map_or(vec![], |loaded| loaded.engine.block_starts());
        let duration = self.duration();
        if let Some(time) = timeline(ui, self.time, duration, &blocks) {
            self.time = time;
            self.playing = false;
            self.needs_render = true;
        }
        if let Some(error) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, error);
        }
        ui.label(if self.message.is_empty() {
            "Drag a shape to move it, with Shift to scale it, with Alt to rotate it."
        } else {
            &self.message
        });
    }

    fn toggle_play(&mut self) {
        if !self.playing && self.time >= self.duration() {
            self.time = 0.0;
        }
        self.playing = !self.playing;
        self.last_tick = Instant::now();
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        let (area, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let Some(texture) = &self.texture else {
            return;
        };
        let rect = fit(area, texture.aspect_ratio());
        // The scene background is usually transparent, for compositing.
        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        ui.painter().image(texture.id(), rect, uv, Color32::WHITE);

        // Preview pixels per screen point.
        let scale = f64::from(texture.size()[0] as f32 / rect.width());
        let to_pixels = |pos: Pos2| {
            Point::new(
                f64::from(pos.x - rect.left()) * scale,
                f64::from(pos.y - rect.top()) * scale,
            )
        };
        if response.drag_started()
            && let Some(origin) = ui.input(|input| input.pointer.press_origin())
        {
            let modifiers = ui.input(|input| input.modifiers);
            let mode = if modifiers.shift {
                Mode::Scale
            } else if modifiers.alt {
                Mode::Rotate
            } else {
                Mode::Move
            };
            self.start_drag(to_pixels(origin), mode);
        } else if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.pick(to_pixels(pos));
        }
        if response.dragged()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.drag_to(to_pixels(pos));
        }
        if response.drag_stopped() {
            self.drag = None;
        }

        let outline = self
            .items
            .iter()
            .find(|item| Some(item.node) == self.selected);
        if let Some(item) = outline {
            let bounds = item.transform.transform_rect_bbox(item.path.bounding_box());
            let to_screen = |x: f64, y: f64| {
                rect.left_top() + egui::vec2((x / scale) as f32, (y / scale) as f32)
            };
            let bounds = Rect::from_min_max(
                to_screen(bounds.x0, bounds.y0),
                to_screen(bounds.x1, bounds.y1),
            );
            ui.painter().rect_stroke(
                bounds.expand(2.0),
                0.0,
                Stroke::new(1.0, Color32::from_rgb(255, 227, 82)),
                egui::StrokeKind::Outside,
            );
        }
    }

    /// Selects the topmost shape at `point` (in preview pixels).
    fn pick(&mut self, point: Point) -> Option<usize> {
        let hit = hit_test(&self.items, point, self.settings.drag.hit_slop);
        self.selected = hit.map(|index| self.items[index].node);
        if hit.is_none() {
            self.message = "Nothing to drag there.".into();
        }
        hit
    }

    fn start_drag(&mut self, point: Point, mode: Mode) {
        self.drag = None;
        self.playing = false;
        let Some(index) = self.pick(point) else {
            return;
        };
        let Some(loaded) = &self.loaded else {
            return;
        };
        let started = Drag::start(
            &loaded.engine,
            &self.items[index],
            mode,
            point,
            self.time,
            self.settings.drag.min_gain,
        );
        match started {
            Ok(drag) => {
                self.message = format!("Dragging {}.", drag.param_names().join(" and "));
                self.drag = Some(drag);
            }
            Err(error) => self.message = error,
        }
    }

    fn drag_to(&mut self, point: Point) {
        let Some(drag) = &self.drag else {
            return;
        };
        let params = drag.params_at(point, self.settings.drag.step);
        let written: Vec<String> = drag
            .param_names()
            .iter()
            .map(|name| format!("{name} = {}", show_value(params[*name])))
            .collect();
        let params_file = params_path(&self.scene);
        match set_params(&params_file, &params) {
            Ok(()) => {
                self.message = format!("Wrote {}.", written.join(", "));
                self.reload();
            }
            Err(error) => {
                self.message = error;
                self.drag = None;
            }
        }
    }
}

impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.reload_if_changed();
        if ui.input(|input| input.key_pressed(egui::Key::Space)) {
            self.toggle_play();
        }
        self.advance_playhead();
        if self.needs_render {
            self.render(ui.ctx());
        }

        egui::Panel::bottom("controls").show(ui, |ui| self.controls(ui));
        egui::CentralPanel::default().show(ui, |ui| self.preview(ui));

        if self.playing {
            ui.ctx().request_repaint();
        } else {
            let poll = std::time::Duration::from_secs_f64(self.settings.watch.poll_seconds);
            ui.ctx().request_repaint_after(poll);
        }
    }
}

fn load(scene: &Path, project: Option<&Path>) -> Result<Loaded, String> {
    let config = Config::find_for(scene)?;
    let markers = match project {
        Some(project) => anicode_shotcut::clip_markers(project, scene)?,
        None => HashMap::new(),
    };
    let engine = load_scene(scene, &config, &markers)?;
    Ok(Loaded { engine, config })
}

fn show_value(value: ParamValue) -> String {
    match value {
        ParamValue::Number(number) => number.to_string(),
        ParamValue::Vec(x, y) => format!("{{ {x}, {y} }}"),
    }
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

/// The largest rectangle with this width to height ratio that fits in
/// `area`, centered.
fn fit(area: Rect, aspect: f32) -> Rect {
    let size = if area.width() / area.height() > aspect {
        egui::vec2(area.height() * aspect, area.height())
    } else {
        egui::vec2(area.width(), area.width() / aspect)
    };
    Rect::from_center_size(area.center(), size)
}

/// A scrub bar with a tick and name at the start of each block. Returns the
/// time the user clicked or dragged to.
fn timeline(ui: &mut egui::Ui, time: f64, duration: f64, blocks: &[(String, f64)]) -> Option<f64> {
    let size = egui::vec2(ui.available_width(), 36.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.rect_filled(rect, 2.0, visuals.extreme_bg_color);

    let x_of = |seconds: f64| {
        let fraction = if duration > 0.0 {
            seconds / duration
        } else {
            0.0
        };
        rect.left() + rect.width() * fraction as f32
    };
    let tick_color = visuals.weak_text_color();
    for (name, start) in blocks {
        let x = x_of(*start);
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, tick_color),
        );
        painter.text(
            Pos2::new(x + 3.0, rect.top() + 2.0),
            Align2::LEFT_TOP,
            name,
            FontId::proportional(11.0),
            tick_color,
        );
    }
    let x = x_of(time);
    painter.line_segment(
        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
        Stroke::new(2.0, Color32::from_rgb(255, 134, 47)),
    );

    let pointer = response.interact_pointer_pos()?;
    let fraction = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    Some(f64::from(fraction) * duration)
}
