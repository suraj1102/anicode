//! Runs a Lua scene file and evaluates it at any point in time.

mod bindings;
mod config;

pub use config::{Config, load_scene};

use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use anicode_core::{DrawItem, Frame};
use mlua::{Lua, Table};

use bindings::State;

const PRELUDE: &str = include_str!("../../../lua/stdlib/prelude.lua");

pub struct Settings {
    /// Scene units visible from the bottom to the top of the frame.
    pub frame_height: f64,
    /// Length of an `at` block whose marker is not set.
    pub default_block_seconds: f64,
}

pub struct Engine {
    lua: Lua,
    state: Rc<State>,
    settings: Settings,
}

impl Engine {
    /// Runs the scene file. `markers` maps `at` block names to start times
    /// in seconds. Errors read like `scene.lua:12: message`.
    pub fn load(
        path: &Path,
        markers: &HashMap<String, f64>,
        settings: Settings,
    ) -> Result<Engine, String> {
        let source = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let module_dir = path.parent().unwrap_or(Path::new("."));
        Engine::from_source(
            &source,
            &path.display().to_string(),
            module_dir,
            markers,
            settings,
        )
    }

    fn from_source(
        source: &str,
        name: &str,
        module_dir: &Path,
        markers: &HashMap<String, f64>,
        settings: Settings,
    ) -> Result<Engine, String> {
        let lua = Lua::new();
        let state = Rc::new(State::default());
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
        state
            .scene
            .borrow_mut()
            .resolve(markers, settings.default_block_seconds)
            .map_err(|error| format!("{name}: {error}"))?;

        Ok(Engine {
            lua,
            state,
            settings,
        })
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
        let settings = Settings {
            frame_height: 8.0,
            default_block_seconds: 2.0,
        };
        Engine::from_source(
            source,
            "test.lua",
            Path::new("."),
            &HashMap::new(),
            settings,
        )
    }

    fn first_fill_alpha(engine: &Engine, time: f64) -> f32 {
        engine.display_list(time, 160, 90).unwrap()[0].fill.unwrap()[3]
    }

    #[test]
    fn fade_in_runs_over_its_block() {
        let engine = load(
            r#"
            local box = Rect { fill = RED }
            at "intro" { fade_in(box):ease("linear") }
            "#,
        )
        .unwrap();
        assert_eq!(engine.duration(), 2.0);
        assert!(engine.display_list(0.0, 160, 90).unwrap().is_empty());
        assert_eq!(first_fill_alpha(&engine, 1.0), 0.5);
        assert_eq!(first_fill_alpha(&engine, 2.0), 1.0);
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
    fn animations_need_an_at_block() {
        let error = load("local c = Circle {}\nfade_in(c)").err().unwrap();
        assert_eq!(
            error,
            "test.lua:2: fade_in must be called inside an at block"
        );
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
    fn unknown_shape_options_are_rejected() {
        let error = load("Rect { widht = 2 }").err().unwrap();
        assert!(error.contains("Rect has no option widht"), "{error}");
    }
}
