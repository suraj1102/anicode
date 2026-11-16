//! Runs a Lua scene file and evaluates it at any point in time.

mod bindings;
mod config;
mod params;

pub use anicode_core::ParamPart;
pub use config::{Config, load_scene};
pub use params::{ParamValue, Params, params_path, read_params, write_params};

use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use anicode_core::{Control, DrawItem, Frame, NodeId, ParamUse};
use mlua::{Lua, Table};

use bindings::State;

const PRELUDE: &str = include_str!("../../../lua/stdlib/prelude.lua");

pub struct Settings {
    /// Scene units visible from the bottom to the top of the frame.
    pub frame_height: f64,
    /// Length of an `at` block whose marker is not set.
    pub default_block_seconds: f64,
    /// Length of an animation without `:over(seconds)`.
    pub default_animation_seconds: f64,
}

/// The param, or one part of a vec param, that sets a node property.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamRef {
    pub name: String,
    pub part: ParamPart,
    /// Changing the param by 1 changes the property by this much at the
    /// time asked about.
    pub gain: f64,
}

pub struct Engine {
    lua: Lua,
    state: Rc<State>,
    settings: Settings,
}

impl Engine {
    /// Runs the scene file, with the values in its params file if it has
    /// one. `markers` maps `at` block names to start times in seconds.
    /// Errors read like `scene.lua:12: message`.
    pub fn load(
        path: &Path,
        markers: &HashMap<String, f64>,
        settings: Settings,
    ) -> Result<Engine, String> {
        let source = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        // A bare file name has an empty parent, which would make `require`
        // search the filesystem root.
        let absolute =
            std::path::absolute(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let module_dir = absolute.parent().unwrap_or(Path::new("/"));
        let params_file = params_path(path);
        let params = if params_file.is_file() {
            read_params(&params_file)?
        } else {
            Params::new()
        };
        Engine::from_source(
            &source,
            &path.display().to_string(),
            module_dir,
            markers,
            params,
            settings,
        )
    }

    fn from_source(
        source: &str,
        name: &str,
        module_dir: &Path,
        markers: &HashMap<String, f64>,
        params: Params,
        settings: Settings,
    ) -> Result<Engine, String> {
        let lua = Lua::new();
        let state = Rc::new(State::default());
        *state.file_params.borrow_mut() = params;
        let to_text = |error: mlua::Error| error_text(&error);

        bindings::register(&lua, &state).map_err(to_text)?;
        lua.load(PRELUDE)
            .set_name("@prelude.lua")
            .exec()
            .map_err(to_text)?;
        set_module_path(&lua, module_dir).map_err(to_text)?;
        lua.load(source)
            .set_name(format!("@{name}"))
            .exec()
            .map_err(to_text)?;

        if state.current_block.get().is_some() {
            return Err(format!("{name}: the last at block has no {{ }} after it"));
        }
        if let Some(pending) = state.pending_play.get() {
            let location = pending
                .line
                .map_or(name.to_string(), |line| format!("{name}:{line}"));
            return Err(format!(
                "{location}: {} is not inside an at or play block",
                pending.action
            ));
        }
        state
            .scene
            .borrow_mut()
            .resolve(
                markers,
                settings.default_block_seconds,
                settings.default_animation_seconds,
            )
            .map_err(|error| format!("{name}: {error}"))?;

        Ok(Engine {
            lua,
            state,
            settings,
        })
    }

    /// Each `at` and `play` block's name and start time in seconds.
    pub fn block_starts(&self) -> Vec<(String, f64)> {
        self.state.scene.borrow().block_starts()
    }

    /// The params the scene used, with their values from the params file or
    /// their defaults.
    pub fn params(&self) -> Params {
        self.state.used_params.borrow().iter().cloned().collect()
    }

    /// Names a node for messages, like "Circle on line 3".
    pub fn node_label(&self, node: NodeId) -> String {
        self.state.node_labels.borrow()[node].clone()
    }

    /// The param that sets `property` of `node` at `time`: the one behind
    /// the animation that last started, or behind the base value. When no
    /// param sets it, the error says what does.
    pub fn param_for(&self, node: NodeId, property: &str, time: f64) -> Result<ParamRef, String> {
        let scene = self.state.scene.borrow();
        let label = self.node_label(node);
        let id = scene.nodes[node]
            .property(property)
            .ok_or_else(|| format!("{label} has no {property}"))?;
        match scene.properties.control(id, time) {
            Control::Param {
                param: ParamUse { param, part },
                gain,
            } => Ok(ParamRef {
                name: self.state.used_params.borrow()[param].0.clone(),
                part,
                gain,
            }),
            Control::Derive => Err(format!(
                "{label}: {property} is computed by derive, so no param sets it"
            )),
            Control::Code { block: None } => Err(format!(
                "{label}: {property} is a number in the code; set it with param() to drag it"
            )),
            Control::Code { block: Some(block) } => Err(format!(
                "{label}: {property} is animated in block \"{}\" to a number in the code; \
                 use param() for the target to drag it",
                scene.blocks[block].name
            )),
        }
    }

    /// Scene length in seconds.
    pub fn duration(&self) -> f64 {
        self.state.scene.borrow().duration()
    }

    /// The shapes to draw at `time` seconds, in pixel space.
    pub fn display_list(
        &self,
        time: f64,
        width: u32,
        height: u32,
    ) -> Result<Vec<DrawItem>, String> {
        *self.state.frame.borrow_mut() = Some(Frame::new(time));
        let result = {
            let scene = self.state.scene.borrow();
            let mut value = |id| bindings::property_value(&self.lua, &self.state, id);
            scene.display_list(&mut value, width, height, self.settings.frame_height)
        };
        *self.state.frame.borrow_mut() = None;
        result.map_err(|error| error_text(&error))
    }
}

/// One line, `file:line: message`, for the error banner. Errors raised by
/// Rust functions carry no location themselves, so it comes from the first
/// Lua frame in their traceback.
fn error_text(error: &mlua::Error) -> String {
    match error {
        mlua::Error::RuntimeError(message) | mlua::Error::SyntaxError { message, .. } => {
            message.lines().next().unwrap_or_default().to_string()
        }
        mlua::Error::CallbackError { traceback, cause } => {
            let message = error_text(cause);
            let location = traceback
                .lines()
                .map(str::trim)
                .find(|line| !line.starts_with("[C]") && !line.starts_with("stack traceback"))
                .and_then(|line| {
                    let mut parts = line.splitn(3, ':');
                    Some(format!("{}:{}", parts.next()?, parts.next()?))
                });
            match location {
                Some(location) => format!("{location}: {message}"),
                None => message,
            }
        }
        other => other.to_string(),
    }
}

/// Lets `require "name"` find files next to the scene and in its `lib` folder.
fn set_module_path(lua: &Lua, module_dir: &Path) -> mlua::Result<()> {
    let package: Table = lua.globals().get("package")?;
    let current: String = package.get("path")?;
    let dir = module_dir.display();
    package.set("path", format!("{dir}/?.lua;{dir}/lib/?.lua;{current}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(source: &str) -> Result<Engine, String> {
        load_with(source, Params::new())
    }

    fn load_with(source: &str, params: Params) -> Result<Engine, String> {
        let settings = Settings {
            frame_height: 8.0,
            default_block_seconds: 2.0,
            default_animation_seconds: 1.0,
        };
        Engine::from_source(
            source,
            "test.lua",
            Path::new("."),
            &HashMap::new(),
            params,
            settings,
        )
    }

    fn first_fill_alpha(engine: &Engine, time: f64) -> f32 {
        engine.display_list(time, 160, 90).unwrap()[0].fill.unwrap()[3]
    }

    #[test]
    fn fade_in_runs_for_the_default_length() {
        let engine = load(
            r#"
            local box = Rect { fill = RED }
            at "intro" { fade_in(box):ease("linear") }
            "#,
        )
        .unwrap();
        assert_eq!(engine.duration(), 2.0);
        assert!(engine.display_list(0.0, 160, 90).unwrap().is_empty());
        assert_eq!(first_fill_alpha(&engine, 0.5), 0.5);
        assert_eq!(first_fill_alpha(&engine, 1.0), 1.0);
    }

    #[test]
    fn derive_follows_another_value() {
        let engine = load(
            r#"
            local level = Value(0)
            local led = Circle { fill = RED }
            led.opacity = derive(function() return level * 0.5 end)
            at "turn" { animate(level, 1):over(1):ease("linear") }
            "#,
        )
        .unwrap();
        assert_eq!(first_fill_alpha(&engine, 0.5), 0.25);
        assert_eq!(first_fill_alpha(&engine, 1.5), 0.5);
    }

    #[test]
    fn derive_cycles_are_reported() {
        let engine = load(
            r#"
            local a = Circle {}
            a.opacity = derive(function() return a.opacity:get() end)
            "#,
        )
        .unwrap();
        let error = engine.display_list(0.0, 160, 90).unwrap_err();
        assert!(error.contains("depends on its own value"), "{error}");
    }

    #[test]
    fn errors_name_the_file_and_line() {
        let error = load("local a = 1\nlocal b = nil + 1\n").err().unwrap();
        assert!(error.starts_with("test.lua:2:"), "{error}");
    }

    #[test]
    fn animations_need_an_at_or_play_block() {
        let error = load("local c = Circle {}\nfade_in(c)").err().unwrap();
        assert_eq!(
            error,
            "test.lua:2: fade_in is not inside an at or play block"
        );
    }

    fn opacity_at(engine: &Engine, time: f64) -> Vec<f32> {
        engine
            .display_list(time, 160, 90)
            .unwrap()
            .iter()
            .map(|item| item.fill.unwrap()[3])
            .collect()
    }

    #[test]
    fn play_blocks_follow_each_other() {
        let engine = load(
            r#"
            local a = Rect { fill = RED }
            local b = Rect { fill = BLUE }
            play { fade_in(a):ease("linear") }
            play { fade_in(b):ease("linear"), seconds = 3 }
            play { fade_out(a):ease("linear") }
            "#,
        )
        .unwrap();
        // Blocks: 0-2 s, 2-5 s (seconds = 3), 5-7 s. Animations last 1 s.
        assert_eq!(engine.duration(), 7.0);
        assert_eq!(opacity_at(&engine, 0.5), [0.5]);
        assert_eq!(opacity_at(&engine, 2.5), [1.0, 0.5]);
        assert_eq!(opacity_at(&engine, 5.5), [0.5, 1.0]);
    }

    #[test]
    fn play_and_at_mix_and_long_animations_extend_blocks() {
        let engine = load(
            r#"
            local a = Rect { fill = RED }
            play { fade_in(a):over(3) }
            at "next" { a:shift(RIGHT), seconds = 1 }
            play { seconds = 2 }
            "#,
        )
        .unwrap();
        // 0-3 s (longest animation), "next" 3-4 s, then a 2 s pause.
        assert_eq!(engine.duration(), 6.0);
        let starts = engine.block_starts();
        assert_eq!(
            starts,
            [
                ("1".to_string(), 0.0),
                ("next".to_string(), 3.0),
                ("2".to_string(), 4.0)
            ]
        );
    }

    #[test]
    fn methods_inside_play_are_rejected() {
        let error = load("local a = Circle {}\nplay { a:shift(UP) }")
            .err()
            .unwrap();
        assert!(
            error.starts_with("test.lua:2: a method like box:shift(UP)"),
            "{error}"
        );
    }

    #[test]
    fn unknown_block_options_are_rejected() {
        let error = load("local a = Circle {}\nplay { fade_in(a), secs = 2 }")
            .err()
            .unwrap();
        assert_eq!(error, "test.lua:2: play has no option secs");
    }

    #[test]
    fn methods_set_values_outside_blocks_and_animate_inside() {
        let engine = load(
            r#"
            local box = Rect { fill = RED }
            box:shift(RIGHT * 2)
            box.label = "kept"
            assert(box.label == "kept")
            assert(box.x:get() == 2)
            at "move" { box:shift(UP):scale(2) }
            "#,
        )
        .unwrap();
        let item = &engine.display_list(2.0, 160, 90).unwrap()[0];
        // 160x90 frame, 8 units tall: 11.25 pixels per unit, origin at (80, 45).
        let center = item.transform * anicode_core::kurbo::Point::ZERO;
        assert_eq!((center.x, center.y), (80.0 + 2.0 * 11.25, 45.0 - 11.25));
    }

    #[test]
    fn animating_a_derived_value_is_rejected() {
        let error = load(
            r#"
            local c = Circle {}
            c.opacity = derive(function() return 1 end)
            at "intro" { fade_in(c) }
            "#,
        )
        .err()
        .unwrap();
        assert!(
            error.starts_with("test.lua: at \"intro\" animates"),
            "{error}"
        );
    }

    #[test]
    fn math_errors_name_the_line() {
        let error = load("local ok = Tex { [[\\frac{1}{2}]] }\nlocal bad = Math { \"x = #nope\" }")
            .err()
            .unwrap();
        assert!(error.starts_with("test.lua:2: math: "), "{error}");
    }

    #[test]
    fn pos_with_x_or_y_is_rejected() {
        let error = load("Circle { pos = vec(1, 2), y = -2 }").err().unwrap();
        assert!(
            error.ends_with("Circle has both pos and y; use one"),
            "{error}"
        );
    }

    #[test]
    fn unknown_shape_options_are_rejected() {
        let error = load("Rect { widht = 2 }").err().unwrap();
        assert!(error.contains("Rect has no option widht"), "{error}");
    }

    fn center(engine: &Engine, time: f64) -> (f64, f64) {
        let item = &engine.display_list(time, 160, 90).unwrap()[0];
        let center = item.transform * anicode_core::kurbo::Point::ZERO;
        // 11.25 pixels per unit, origin at (80, 45), y up.
        ((center.x - 80.0) / 11.25, (45.0 - center.y) / 11.25)
    }

    const DOT: &str = r#"
        local dot = Circle { pos = param("dot.start", vec(1, 0)), fill = RED }
        play { move_to(dot, param("dot.end", vec(-1, 0))) }
        play { shift(dot, UP) }
        play { scale(dot, param("dot.grow", 2)) }
    "#;

    #[test]
    fn params_use_defaults_or_the_file() {
        let engine = load(DOT).unwrap();
        assert_eq!(center(&engine, 0.0), (1.0, 0.0));
        assert_eq!(center(&engine, 2.0), (-1.0, 0.0));
        assert_eq!(engine.params()["dot.grow"], ParamValue::Number(2.0));

        let file = Params::from([("dot.end".to_string(), ParamValue::Vec(3.0, 2.0))]);
        let engine = load_with(DOT, file).unwrap();
        assert_eq!(center(&engine, 2.0), (3.0, 2.0));
        assert_eq!(center(&engine, 4.0), (3.0, 3.0));
        assert_eq!(engine.params()["dot.end"], ParamValue::Vec(3.0, 2.0));
    }

    #[test]
    fn params_file_values_must_match_the_default_kind() {
        let file = Params::from([("dot.grow".to_string(), ParamValue::Vec(1.0, 1.0))]);
        let error = load_with(DOT, file).err().unwrap();
        assert!(
            error.ends_with("gives dot.grow a vec, but its default is a number"),
            "{error}"
        );
    }

    #[test]
    fn param_for_finds_the_param_at_the_playhead() {
        let engine = load(DOT).unwrap();
        let param = |property, time| engine.param_for(0, property, time);
        let named = |name: &str, part, gain| {
            Ok(ParamRef {
                name: name.to_string(),
                part,
                gain,
            })
        };
        assert_eq!(param("x", 0.0), named("dot.start", ParamPart::X, 1.0));
        assert_eq!(param("y", 1.0), named("dot.end", ParamPart::Y, 1.0));
        // Half way through move_to, the target has moved the dot half way.
        assert_eq!(param("y", 0.5), named("dot.end", ParamPart::Y, 0.5));
        // shift by a number in code follows where move_to left the dot.
        assert_eq!(param("y", 3.0), named("dot.end", ParamPart::Y, 1.0));
        let grow = named("dot.grow", ParamPart::Number, 1.0);
        assert_eq!(param("scale", 5.0), grow);
        let error = param("scale", 0.0).unwrap_err();
        assert_eq!(
            error,
            "Circle on line 2: scale is a number in the code; set it with param() to drag it"
        );
    }

    #[test]
    fn param_numbers_work_in_arithmetic() {
        let engine = load(
            r#"
            local size = param("size", 0.5)
            assert(size * 2 == 1 and 1 + size == 1.5 and -size == -0.5)
            Circle { r = size, x = param("x", 2), fill = RED }
            "#,
        )
        .unwrap();
        assert_eq!(center(&engine, 0.0), (2.0, 0.0));
    }
}
