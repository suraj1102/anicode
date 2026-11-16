//! Dragging a shape in the preview: turns cursor movement into new values
//! for the params that place the shape.

use anicode_core::DrawItem;
use anicode_core::kurbo::{Affine, Point};
use anicode_lua::{Engine, ParamPart, ParamRef, ParamValue, Params};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Move,
    Scale,
    Rotate,
}

pub struct Drag {
    mode: Mode,
    /// The params behind the properties this drag changes, in the order of
    /// `properties(mode)`.
    params: Vec<ParamRef>,
    /// Every param's value when the drag started.
    start_values: Params,
    /// The params file when the drag started. Only dragged params change.
    start_file: Params,
    /// Maps preview pixels to the space the node's x and y live in.
    to_parent: Affine,
    start: Point,
    /// The node's origin, which scaling and rotating turn around.
    center: Point,
}

fn properties(mode: Mode) -> &'static [&'static str] {
    match mode {
        Mode::Move => &["x", "y"],
        Mode::Scale => &["scale"],
        Mode::Rotate => &["rotation"],
    }
}

impl Drag {
    /// Starts dragging `item` from `cursor` (in preview pixels). Fails with
    /// a message when no param sets one of the properties the drag changes.
    pub fn start(
        engine: &Engine,
        item: &DrawItem,
        mode: Mode,
        cursor: Point,
        time: f64,
        file: Params,
    ) -> Result<Drag, String> {
        let params = properties(mode)
            .iter()
            .map(|property| engine.param_for(item.node, property, time))
            .collect::<Result<Vec<_>, _>>()?;
        let to_parent = item.parent_transform.inverse();
        Ok(Drag {
            mode,
            params,
            start_values: engine.params(),
            start_file: file,
            to_parent,
            start: to_parent * cursor,
            center: to_parent * (item.transform * Point::ZERO),
        })
    }

    /// The names of the params this drag changes.
    pub fn param_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.params.iter().map(|param| &*param.name).collect();
        names.dedup();
        names
    }

    /// The params file with the cursor at `cursor`. Values are rounded to
    /// `step`.
    pub fn params_at(&self, cursor: Point, step: f64) -> Params {
        let cursor = self.to_parent * cursor;
        let from_start = self.start - self.center;
        let from_cursor = cursor - self.center;
        let changes: Vec<Box<dyn Fn(f64) -> f64>> = match self.mode {
            Mode::Move => {
                let moved = cursor - self.start;
                vec![
                    Box::new(move |x| x + moved.x),
                    Box::new(move |y| y + moved.y),
                ]
            }
            // Scale and rotation params are factors or angles, so a change
            // works the same on a base value, a target or a relative change.
            Mode::Scale => {
                let ratio = if from_start.hypot() > 0.0 {
                    from_cursor.hypot() / from_start.hypot()
                } else {
                    1.0
                };
                vec![Box::new(move |scale| scale * ratio)]
            }
            Mode::Rotate => {
                let turn = from_cursor.atan2() - from_start.atan2();
                vec![Box::new(move |angle| angle + turn)]
            }
        };

        let mut values = self.start_values.clone();
        for (param, change) in self.params.iter().zip(changes) {
            let value = values[&param.name];
            values.insert(param.name.clone(), changed(value, param.part, change));
        }
        let mut file = self.start_file.clone();
        for name in self.param_names() {
            file.insert(name.to_string(), rounded(values[name], step));
        }
        file
    }
}

fn changed(value: ParamValue, part: ParamPart, change: impl Fn(f64) -> f64) -> ParamValue {
    match (value, part) {
        (ParamValue::Number(number), _) => ParamValue::Number(change(number)),
        (ParamValue::Vec(x, y), ParamPart::X) => ParamValue::Vec(change(x), y),
        (ParamValue::Vec(x, y), ParamPart::Y) => ParamValue::Vec(x, change(y)),
        (vector, ParamPart::Number) => vector,
    }
}

fn rounded(value: ParamValue, step: f64) -> ParamValue {
    // Dividing by a whole number of steps keeps 1.234 from printing as
    // 1.2340000000000002.
    let steps = 1.0 / step;
    let round = |number: f64| (number * steps).round() / steps;
    match value {
        ParamValue::Number(number) => ParamValue::Number(round(number)),
        ParamValue::Vec(x, y) => ParamValue::Vec(round(x), round(y)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anicode_lua::Settings;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const WIDTH: u32 = 160;
    const HEIGHT: u32 = 90;
    /// 160x90 pixels, 8 units tall: 11.25 pixels per unit, origin at (80, 45).
    fn pixel(x: f64, y: f64) -> Point {
        Point::new(80.0 + x * 11.25, 45.0 - y * 11.25)
    }

    fn load(source: &str) -> Engine {
        // Tests run in parallel, so each load gets its own file.
        static LOADS: AtomicUsize = AtomicUsize::new(0);
        let count = LOADS.fetch_add(1, Ordering::Relaxed);
        let name = format!("anicode-drag-{}-{count}.lua", std::process::id());
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, source).unwrap();
        let settings = Settings {
            frame_height: 8.0,
            default_block_seconds: 2.0,
            default_animation_seconds: 1.0,
        };
        let engine = Engine::load(&path, &HashMap::new(), settings).unwrap();
        std::fs::remove_file(&path).unwrap();
        engine
    }

    fn drag(engine: &Engine, mode: Mode, time: f64, from: Point) -> Result<Drag, String> {
        let items = engine.display_list(time, WIDTH, HEIGHT).unwrap();
        Drag::start(engine, &items[0], mode, from, time, Params::new())
    }

    const SCENE: &str = r#"
        local group = Group { x = 1 }
        local dot = group:add(Circle { pos = param("dot.start", vec(1, 0)), fill = RED })
        play { move_to(dot, param("dot.end", vec(0, 2))) }
        play { scale(dot, param("dot.grow", 2)) }
        play { rotate(dot, 1) }
    "#;

    #[test]
    fn moving_changes_the_param_behind_the_position() {
        let engine = load(SCENE);
        // Before the move, the dot sits at dot.start inside the group.
        let drag = drag(&engine, Mode::Move, 0.0, pixel(2.0, 0.0)).unwrap();
        let file = drag.params_at(pixel(2.5, -1.0), 0.001);
        assert_eq!(
            file,
            Params::from([("dot.start".into(), ParamValue::Vec(1.5, -1.0))])
        );
    }

    #[test]
    fn moving_after_an_animation_changes_its_target() {
        let engine = load(SCENE);
        let drag = drag(&engine, Mode::Move, 1.5, pixel(1.0, 2.0)).unwrap();
        let file = drag.params_at(pixel(1.25, 2.0), 0.001);
        assert_eq!(file["dot.end"], ParamValue::Vec(0.25, 2.0));
    }

    #[test]
    fn scaling_multiplies_the_factor() {
        let engine = load(SCENE);
        let drag = drag(&engine, Mode::Scale, 3.5, pixel(2.0, 2.0)).unwrap();
        let file = drag.params_at(pixel(1.0, 4.0), 0.001);
        assert_eq!(file["dot.grow"], ParamValue::Number(4.0));
    }

    #[test]
    fn values_without_a_param_are_reported() {
        let engine = load(SCENE);
        let error = drag(&engine, Mode::Rotate, 5.0, pixel(1.0, 2.0))
            .err()
            .unwrap();
        // rotate(dot, 1) turns by a number in code, from a base value in code.
        assert!(
            error.ends_with("rotation is a number in the code; set it with param() to drag it"),
            "{error}"
        );
    }

    #[test]
    fn other_params_in_the_file_are_kept() {
        let engine = load(SCENE);
        let items = engine.display_list(0.0, WIDTH, HEIGHT).unwrap();
        let file = Params::from([("other".into(), ParamValue::Number(3.0))]);
        let drag = Drag::start(&engine, &items[0], Mode::Move, pixel(2.0, 0.0), 0.0, file).unwrap();
        let file = drag.params_at(pixel(2.0, 0.0), 0.001);
        assert_eq!(file.len(), 2);
        assert_eq!(file["other"], ParamValue::Number(3.0));
    }
}
