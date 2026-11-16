//! Dragging a shape in the preview: turns cursor movement into new values
//! for the params that place the shape.

use anicode_core::NodeId;
use anicode_core::kurbo::{Affine, Point};
use anicode_lua::{Engine, ParamPart, ParamRef, ParamValue, Params};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Move,
    Scale,
    Rotate,
}

pub struct Drag {
    /// Names the node being dragged, like "Group on line 2".
    pub label: String,
    mode: Mode,
    /// The params behind the properties this drag changes, in the order of
    /// `properties(mode)`.
    params: Vec<ParamRef>,
    /// Every param's value when the drag started.
    start_values: Params,
    /// Maps preview pixels to the space the node's x and y live in.
    to_parent: Affine,
    start: Point,
    /// The node's origin, which scaling and rotating turn around.
    center: Point,
    /// The node's own scale at the drag's time.
    scale: f64,
}

fn properties(mode: Mode) -> &'static [&'static str] {
    match mode {
        Mode::Move => &["x", "y"],
        Mode::Scale => &["scale"],
        Mode::Rotate => &["rotation"],
    }
}

impl Drag {
    /// Starts dragging `node` from `cursor` (in pixels of a `size` frame).
    /// If no param places the node, its parents are tried, nearest first,
    /// so a shape inside a component drags the component. Fails with the
    /// node's own message when none of them can be dragged.
    pub fn start(
        engine: &Engine,
        node: NodeId,
        mode: Mode,
        cursor: Point,
        time: f64,
        size: (u32, u32),
        min_gain: f64,
    ) -> Result<Drag, String> {
        let mut first_error = None;
        let mut next = Some(node);
        while let Some(node) = next {
            match Drag::start_on(engine, node, mode, cursor, time, size, min_gain) {
                Ok(drag) => return Ok(drag),
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
            next = engine.parent(node);
        }
        Err(first_error.expect("the loop runs at least once"))
    }

    /// Fails when no param sets one of the properties the drag changes, or
    /// when a param's gain is below `min_gain`, because then the shape would
    /// barely follow the cursor.
    fn start_on(
        engine: &Engine,
        node: NodeId,
        mode: Mode,
        cursor: Point,
        time: f64,
        (width, height): (u32, u32),
        min_gain: f64,
    ) -> Result<Drag, String> {
        let label = engine.node_label(node);
        let mut params = vec![];
        for property in properties(mode) {
            let param = engine.param_for(node, property, time)?;
            if param.gain.abs() < min_gain {
                return Err(format!(
                    "{label}: the animation that sets {property} has barely started; \
                     scrub later to drag it"
                ));
            }
            params.push(param);
        }
        let (parent_transform, transform) = engine.transforms(node, time, width, height)?;
        let to_parent = parent_transform.inverse();
        let scale = (transform.determinant() / parent_transform.determinant())
            .abs()
            .sqrt();
        Ok(Drag {
            label,
            mode,
            params,
            start_values: engine.params(),
            to_parent,
            start: to_parent * cursor,
            center: to_parent * (transform * Point::ZERO),
            scale,
        })
    }
    /// The names of the params this drag changes.
    pub fn param_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.params.iter().map(|param| &*param.name).collect();
        names.dedup();
        names
    }

    /// The dragged params' values with the cursor at `cursor`, rounded to
    /// `step`.
    pub fn params_at(&self, cursor: Point, step: f64) -> Params {
        let cursor = self.to_parent * cursor;
        let from_start = self.start - self.center;
        let from_cursor = cursor - self.center;
        // How much each property should change, in the order of `params`.
        let changes = match self.mode {
            Mode::Move => {
                let moved = cursor - self.start;
                vec![moved.x, moved.y]
            }
            Mode::Scale => {
                let ratio = if from_start.hypot() > 0.0 {
                    from_cursor.hypot() / from_start.hypot()
                } else {
                    1.0
                };
                vec![self.scale * (ratio - 1.0)]
            }
            Mode::Rotate => vec![from_cursor.atan2() - from_start.atan2()],
        };

        let mut values = self.start_values.clone();
        for (param, change) in self.params.iter().zip(changes) {
            let value = values[&param.name];
            let new_value = changed(value, param.part, |old| old + change / param.gain);
            values.insert(param.name.clone(), new_value);
        }
        self.param_names()
            .into_iter()
            .map(|name| (name.to_string(), rounded(values[name], step)))
            .collect()
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

pub fn rounded(value: ParamValue, step: f64) -> ParamValue {
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

    const MIN_GAIN: f64 = 0.05;
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
        let size = (WIDTH, HEIGHT);
        Drag::start(
            engine,
            first_shape(engine),
            mode,
            from,
            time,
            size,
            MIN_GAIN,
        )
    }

    fn first_shape(engine: &Engine) -> NodeId {
        engine.display_list(0.0, WIDTH, HEIGHT).unwrap()[0].node
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
    fn moving_during_an_animation_follows_the_cursor() {
        let engine = load(SCENE);
        // Half way through move_to, the dot is half way from (2, 0) to (1, 2).
        let drag = drag(&engine, Mode::Move, 0.5, pixel(1.5, 1.0)).unwrap();
        let file = drag.params_at(pixel(2.0, 1.0), 0.001);
        assert_eq!(file["dot.end"], ParamValue::Vec(1.0, 2.0));
    }

    #[test]
    fn moving_just_after_an_animation_starts_is_refused() {
        let engine = load(SCENE);
        let error = drag(&engine, Mode::Move, 0.01, pixel(2.0, 0.0))
            .err()
            .unwrap();
        assert!(error.contains("x has barely started"), "{error}");
    }

    #[test]
    fn shapes_without_a_param_drag_their_group() {
        let engine = load(
            r#"
            local group = Group { pos = param("group.pos", vec(1, 0)), scale = 2 }
            group:add(Circle { x = 0.5, fill = RED })
            "#,
        );
        let drag = drag(&engine, Mode::Move, 0.0, pixel(2.0, 0.0)).unwrap();
        assert_eq!(drag.label, "Group on line 2");
        let file = drag.params_at(pixel(3.0, 1.0), 0.001);
        assert_eq!(file["group.pos"], ParamValue::Vec(2.0, 1.0));
    }

    #[test]
    fn followers_drag_by_their_offset() {
        let engine = load(
            r#"
            local dot = Circle { r = 0.2, fill = RED }
            local tag = Circle { r = 0.2, fill = RED }
            tag:follow(dot, param("tag.offset", vec(0, 1)))
            play { move_to(dot, vec(2, 0)):ease("linear") }
            "#,
        );
        // Half way through the move the tag is at (1, 1).
        let drag = Drag::start(
            &engine,
            1,
            Mode::Move,
            pixel(1.0, 1.0),
            0.5,
            (WIDTH, HEIGHT),
            MIN_GAIN,
        )
        .unwrap();
        let file = drag.params_at(pixel(1.5, 2.0), 0.001);
        assert_eq!(file["tag.offset"], ParamValue::Vec(0.5, 2.0));
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
}
