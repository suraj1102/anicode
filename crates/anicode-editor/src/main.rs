//! A preview window for a scene: play it, scrub it, and see edits to the
//! scene and its params file as soon as they are saved. Dragging a shape
//! changes the params that place it and writes them to the params file.

mod drag;
mod history;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use anicode_core::kurbo::{Point, Shape as _};
use anicode_core::{DrawItem, NodeId, hit_test};
use anicode_lua::{
    Config, Engine, ParamValue, Params, load_scene, params_path, read_params, save_params,
    set_params,
};
use anicode_render::CpuRenderer;
use eframe::egui::{
    self, Align2, Color32, FontId, Key, KeyboardShortcut, Modifiers, Pos2, Rect, Sense, Stroke,
    TextureHandle,
};
use serde::Deserialize;

use drag::{Drag, Mode};
use history::History;

const CONFIG_FILE: &str = "config/editor.toml";

#[derive(Deserialize)]
struct EditorConfig {
    preview: PreviewConfig,
    drag: DragConfig,
    watch: WatchConfig,
    history: HistoryConfig,
}

#[derive(Deserialize)]
struct HistoryConfig {
    folder: PathBuf,
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
    history: History,
    /// Where the history is saved, if the project folder was found.
    history_file: Option<PathBuf>,
}

impl Editor {
    fn new(scene: PathBuf, settings: EditorConfig) -> Editor {
        let root = Config::find_for(&scene).ok().map(|config| config.root);
        let project = root
            .as_deref()
            .and_then(|root| anicode_shotcut::find_project(root, &scene));
        let history_file = root.map(|root| history_file(&root, &settings.history.folder, &scene));
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
            history: History::new(Params::new()),
            history_file,
        };
        let params = editor.file_params().unwrap_or_default();
        let saved = editor.history_file.as_deref().filter(|path| path.is_file());
        match saved.map(History::load) {
            Some(Ok(history)) => {
                editor.history = history;
                editor.history.record(params, "edited outside the editor");
            }
            Some(Err(error)) => {
                editor.message = format!("Started a new history: {error}");
                editor.history = History::new(params);
            }
            None => editor.history = History::new(params),
        }
        editor.reload();
        editor
    }

    /// Adds a history state and saves the history.
    fn record(&mut self, params: Params, label: &str) {
        if self.history.record(params, label) {
            self.save_history();
        }
    }

    fn save_history(&mut self) {
        if let Some(path) = &self.history_file
            && let Err(error) = self.history.save(path)
        {
            self.message = error;
        }
    }

    /// What the params file holds now. A missing file holds no params.
    fn file_params(&self) -> Result<Params, String> {
        let path = params_path(&self.scene);
        if path.is_file() {
            read_params(&path)
        } else {
            Ok(Params::new())
        }
    }

    /// Writes the current history state to the params file.
    fn restore(&mut self) {
        self.save_history();
        match save_params(&params_path(&self.scene), self.history.params()) {
            Ok(()) => self.reload(),
            Err(error) => self.message = error,
        }
    }

    fn undo(&mut self) {
        let left = self.history.current();
        if self.history.undo() {
            self.message = format!("Undid {}.", self.history.nodes()[left].label);
            self.restore();
        }
    }

    fn redo(&mut self) {
        if self.history.redo() {
            let label = &self.history.nodes()[self.history.current()].label;
            self.message = format!("Redid {label}.");
            self.restore();
        }
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
            // The editor's own writes update file_times, so a change seen
            // here came from somewhere else.
            if self.drag.is_none()
                && let Ok(params) = self.file_params()
            {
                self.record(params, "edited outside the editor");
            }
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
            ui.separator();
            if ui.button("Undo").clicked() {
                self.undo();
            }
            if ui.button("Redo").clicked() {
                self.redo();
            }
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
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, "preview"));
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
        if response.drag_stopped()
            && let Some(drag) = self.drag.take()
            && let Ok(params) = self.file_params()
        {
            let label = format!("drag {}", drag.param_names().join(" and "));
            self.record(params, &label);
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
        let (Some(loaded), Some(texture)) = (&self.loaded, &self.texture) else {
            return;
        };
        let [width, height] = texture.size();
        let started = Drag::start(
            &loaded.engine,
            self.items[index].node,
            mode,
            point,
            self.time,
            (width as u32, height as u32),
            self.settings.drag.min_gain,
        );
        match started {
            Ok(drag) => {
                self.message = format!(
                    "Dragging {} ({}).",
                    drag.param_names().join(" and "),
                    drag.label
                );
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

        let redo = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        let undo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
        // Redo first: the undo shortcut would also match Cmd+Shift+Z.
        if ui.input_mut(|input| input.consume_shortcut(&redo)) {
            self.redo();
        } else if ui.input_mut(|input| input.consume_shortcut(&undo)) {
            self.undo();
        }

        egui::Panel::bottom("controls").show(ui, |ui| self.controls(ui));
        egui::Panel::right("history").show(ui, |ui| self.history_panel(ui));
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

impl Editor {
    fn history_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("History");
        let mut jump = None;
        egui::ScrollArea::vertical().show(ui, |ui| self.history_rows(ui, 0, 0, &mut jump));
        if let Some(node) = jump {
            self.history.jump(node);
            self.message = format!("Went to {}.", self.history.nodes()[node].label);
            self.restore();
        }
    }

    /// One row per state, oldest first. Rows are indented only where the
    /// tree branches, so a history without branches stays flat.
    fn history_rows(&self, ui: &mut egui::Ui, node: usize, depth: usize, jump: &mut Option<usize>) {
        let entry = &self.history.nodes()[node];
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 12.0);
            let text = format!("{} ({})", entry.label, time_ago(entry.when));
            if ui
                .selectable_label(node == self.history.current(), text)
                .clicked()
            {
                *jump = Some(node);
            }
        });
        let branches = entry.children.len() > 1;
        for &child in &entry.children {
            self.history_rows(ui, child, depth + usize::from(branches), jump);
        }
    }
}

/// `examples/drag.lua` keeps its history in
/// `<root>/<folder>/examples/drag.toml`.
fn history_file(root: &Path, folder: &Path, scene: &Path) -> PathBuf {
    let scene = std::path::absolute(scene).unwrap_or_else(|_| scene.to_path_buf());
    let relative = scene
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| PathBuf::from(scene.file_name().unwrap_or_default()));
    root.join(folder).join(relative).with_extension("toml")
}

fn time_ago(when: SystemTime) -> String {
    let seconds = when.elapsed().map_or(0, |elapsed| elapsed.as_secs());
    match seconds {
        0..60 => format!("{seconds} s ago"),
        60..3600 => format!("{} min ago", seconds / 60),
        _ => format!("{} h ago", seconds / 3600),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// A project folder with the repository's config and one scene.
    fn project(name: &str, scene: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("anicode-editor-{name}-{}", std::process::id()));
        std::fs::create_dir_all(root.join("config")).unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::copy(
            repo.join("config/anicode.toml"),
            root.join("config/anicode.toml"),
        )
        .unwrap();
        std::fs::write(root.join("scene.lua"), scene).unwrap();
        root
    }

    fn open(scene: &Path) -> Harness<'static, Editor> {
        let settings = EditorConfig {
            preview: PreviewConfig { scale: 0.25 },
            drag: DragConfig {
                step: 0.001,
                hit_slop: 4.0,
                min_gain: 0.05,
            },
            // Check the files on every frame.
            watch: WatchConfig { poll_seconds: 0.0 },
            history: HistoryConfig {
                folder: PathBuf::from(".anicode/history"),
            },
        };
        let scene = scene.to_path_buf();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_eframe(|_| Editor::new(scene, settings));
        harness.run_steps(2);
        harness
    }

    /// Where a scene point is drawn, for a frame 8 units tall.
    fn screen_point(harness: &Harness<'static, Editor>, x: f64, y: f64) -> Pos2 {
        let area = harness.get_by_label("preview").rect();
        let image = fit(area, 16.0 / 9.0);
        let points_per_unit = f64::from(image.height()) / 8.0;
        image.center() + egui::vec2((x * points_per_unit) as f32, (-y * points_per_unit) as f32)
    }

    #[test]
    fn dragging_a_shape_writes_its_param() {
        let root = project(
            "drag",
            "Circle { r = 0.5, pos = param(\"dot.pos\", vec(0, 0)), fill = RED }\n",
        );
        let mut harness = open(&root.join("scene.lua"));
        let start = screen_point(&harness, 0.0, 0.0);
        let end = screen_point(&harness, 1.0, 0.5);
        harness.drag_at(start);
        harness.run_steps(1);
        harness.hover_at(end);
        harness.run_steps(1);
        harness.drop_at(end);
        harness.run_steps(1);

        let params = anicode_lua::read_params(&root.join("scene.params.lua")).unwrap();
        let ParamValue::Vec(x, y) = params["dot.pos"] else {
            panic!("dot.pos is not a vec");
        };
        // One screen point is about 0.02 units here.
        assert!((x - 1.0).abs() < 0.03 && (y - 0.5).abs() < 0.03, "{x} {y}");
        assert!(harness.state().message.starts_with("Wrote dot.pos"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn undo_and_redo_restore_the_params_file() {
        let root = project(
            "undo",
            "Circle { r = 0.5, pos = param(\"dot.pos\", vec(0, 0)), fill = RED }\n",
        );
        let params_file = root.join("scene.params.lua");
        let mut harness = open(&root.join("scene.lua"));
        let start = screen_point(&harness, 0.0, 0.0);
        let end = screen_point(&harness, 1.0, 0.0);
        harness.drag_at(start);
        harness.run_steps(1);
        harness.hover_at(end);
        harness.run_steps(1);
        harness.drop_at(end);
        harness.run_steps(1);
        let dragged = anicode_lua::read_params(&params_file).unwrap();
        assert!(dragged.contains_key("dot.pos"));

        harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        harness.run_steps(1);
        assert!(anicode_lua::read_params(&params_file).unwrap().is_empty());
        assert_eq!(harness.state().message, "Undid drag dot.pos.");

        harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        harness.run_steps(1);
        assert_eq!(anicode_lua::read_params(&params_file).unwrap(), dragged);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn history_is_kept_after_reopening() {
        let root = project("reopen", "Circle { r = param(\"size\", 1), fill = RED }\n");
        let scene = root.join("scene.lua");
        let mut harness = open(&scene);
        std::fs::write(root.join("scene.params.lua"), "return { size = 2 }\n").unwrap();
        harness.run_steps(2);
        drop(harness);
        assert!(root.join(".anicode/history/scene.toml").is_file());

        let harness = open(&scene);
        assert_eq!(harness.state().history.nodes().len(), 2);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn edits_outside_the_editor_are_recorded() {
        let root = project("outside", "Circle { r = param(\"size\", 1), fill = RED }\n");
        let mut harness = open(&root.join("scene.lua"));
        std::fs::write(root.join("scene.params.lua"), "return { size = 2 }\n").unwrap();
        harness.run_steps(2);
        let history = &harness.state().history;
        assert_eq!(history.nodes().len(), 2);
        assert_eq!(history.nodes()[1].label, "edited outside the editor");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn script_errors_show_and_keep_the_last_frame() {
        let root = project("error", "Circle { fill = RED }\n");
        let scene = root.join("scene.lua");
        let mut harness = open(&scene);
        std::fs::write(&scene, "Circle { fill = RED }\nlocal broken = nil + 1\n").unwrap();
        harness.run_steps(2);

        let error = harness.state().error.clone().unwrap_or_default();
        assert!(error.contains("scene.lua:2:"), "{error}");
        assert!(harness.query_by_label_contains("scene.lua:2:").is_some());
        assert_eq!(harness.state().items.len(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
