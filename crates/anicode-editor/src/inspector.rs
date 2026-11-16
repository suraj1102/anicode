//! The side panel: the scene's params, what sets the selected object, and
//! the undo history.

use std::process::Command;
use std::time::SystemTime;

use anicode_lua::{ParamPart, ParamValue, Params, params_path, save_params, set_params};
use eframe::egui;

use crate::drag::rounded;
use crate::{Editor, show_value};

/// The properties the Selected section lists.
const PROPERTIES: [&str; 5] = ["x", "y", "scale", "rotation", "opacity"];

impl Editor {
    pub(crate) fn inspector(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::CollapsingHeader::new("Params")
                .default_open(true)
                .show(ui, |ui| self.params_section(ui));
            egui::CollapsingHeader::new("Selected")
                .default_open(true)
                .show(ui, |ui| self.selected_section(ui));
            egui::CollapsingHeader::new("History")
                .default_open(true)
                .show(ui, |ui| self.history_section(ui));
        });
    }

    /// Every param the scene uses. Values in the params file have a button
    /// that removes them, so the default in the code applies again.
    fn params_section(&mut self, ui: &mut egui::Ui) {
        let Some(loaded) = &self.loaded else {
            ui.weak("No scene loaded.");
            return;
        };
        let params = loaded.engine.params();
        if params.is_empty() {
            ui.weak("The scene uses no param().");
            return;
        }
        let speed = self.settings.inspector.speed;
        let in_file = self.history.params().clone();
        let mut edit = None;
        let mut reset = None;
        egui::Grid::new("params").num_columns(3).show(ui, |ui| {
            for (name, value) in &params {
                ui.label(name);
                let (new_value, response) = match *value {
                    ParamValue::Number(mut number) => {
                        let response = ui.add(egui::DragValue::new(&mut number).speed(speed));
                        (ParamValue::Number(number), response)
                    }
                    ParamValue::Vec(mut x, mut y) => {
                        let response = ui
                            .horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut x).speed(speed))
                                    | ui.add(egui::DragValue::new(&mut y).speed(speed))
                            })
                            .inner;
                        (ParamValue::Vec(x, y), response)
                    }
                };
                // A drag writes on every change and adds one history state
                // when it ends. A typed value is finished when it changes.
                if response.changed() {
                    edit = Some((name.clone(), new_value, !response.dragged()));
                } else if response.drag_stopped() {
                    edit = Some((name.clone(), new_value, true));
                }
                if in_file.contains_key(name) {
                    let button = ui.small_button("reset");
                    if button
                        .on_hover_text("Remove it from the params file")
                        .clicked()
                    {
                        reset = Some(name.clone());
                    }
                } else {
                    ui.weak("default");
                }
                ui.end_row();
            }
        });
        if let Some((name, value, finished)) = edit {
            self.edit_param(&name, value, finished);
        }
        if let Some(name) = reset {
            self.reset_param(&name);
        }
    }

    fn edit_param(&mut self, name: &str, value: ParamValue, finished: bool) {
        let value = rounded(value, self.settings.drag.step);
        let changes = Params::from([(name.to_string(), value)]);
        if let Err(error) = set_params(&params_path(&self.scene), &changes) {
            self.message = error;
            return;
        }
        self.message = format!("Wrote {name} = {}.", show_value(value));
        self.reload();
        if finished && let Ok(params) = self.file_params() {
            self.record(params, &format!("edit {name}"));
        }
    }

    fn reset_param(&mut self, name: &str) {
        let mut params = match self.file_params() {
            Ok(params) => params,
            Err(error) => {
                self.message = error;
                return;
            }
        };
        params.remove(name);
        if let Err(error) = save_params(&params_path(&self.scene), &params) {
            self.message = error;
            return;
        }
        self.message = format!("{name} uses its default again.");
        self.reload();
        self.record(params, &format!("reset {name}"));
    }

    /// The selected object's properties at the playhead, and the param
    /// behind each one, or why none is.
    fn selected_section(&mut self, ui: &mut egui::Ui) {
        let (Some(loaded), Some(node)) = (&self.loaded, self.selected) else {
            ui.weak("Click a shape to see what places it.");
            return;
        };
        let engine = &loaded.engine;
        ui.label(engine.node_label(node));
        egui::Grid::new("selected").num_columns(3).show(ui, |ui| {
            for property in PROPERTIES {
                ui.label(property);
                match engine.property_value(node, property, self.time) {
                    Ok(value) => ui.monospace(format!("{value:.3}")),
                    Err(error) => ui.weak("?").on_hover_text(error),
                };
                match engine.param_for(node, property, self.time) {
                    Ok(param) => {
                        let part = match param.part {
                            ParamPart::Number => "",
                            ParamPart::X => ".x",
                            ParamPart::Y => ".y",
                        };
                        ui.label(format!("{}{part}", param.name))
                    }
                    Err(reason) => ui.weak("not a param").on_hover_text(reason),
                };
                ui.end_row();
            }
        });

        let command = &self.settings.inspector.open_command;
        if let Some((file, line)) = engine.node_place(node)
            && !command.is_empty()
            && ui.button("Open in editor").clicked()
        {
            let file = file.display().to_string();
            let line = line.to_string();
            // Split before filling in, so a path with spaces stays one word.
            let words: Vec<String> = command
                .split_whitespace()
                .map(|word| word.replace("{file}", &file).replace("{line}", &line))
                .collect();
            if let Err(error) = Command::new(&words[0]).args(&words[1..]).spawn() {
                self.message = format!("{}: {error}", words[0]);
            }
        }
    }

    fn history_section(&mut self, ui: &mut egui::Ui) {
        let mut jump = None;
        self.history_rows(ui, 0, 0, &mut jump);
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

fn time_ago(when: SystemTime) -> String {
    let seconds = when.elapsed().map_or(0, |elapsed| elapsed.as_secs());
    match seconds {
        0..60 => format!("{seconds} s ago"),
        60..3600 => format!("{} min ago", seconds / 60),
        _ => format!("{} h ago", seconds / 3600),
    }
}
