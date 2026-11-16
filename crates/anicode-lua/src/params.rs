//! Params files: `scene.params.lua` next to `scene.lua`, a plain Lua file
//! that returns a table of numbers and {x, y} pairs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mlua::{Lua, Table, Value};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamValue {
    Number(f64),
    Vec(f64, f64),
}

pub type Params = BTreeMap<String, ParamValue>;

/// `examples/dot.lua` keeps its params in `examples/dot.params.lua`.
pub fn params_path(scene: &Path) -> PathBuf {
    let stem = scene.file_stem().unwrap_or_default().to_string_lossy();
    scene.with_file_name(format!("{stem}.params.lua"))
}

pub fn read_params(path: &Path) -> Result<Params, String> {
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let name = path.display().to_string();
    let lua = Lua::new();
    let table: Table = lua
        .load(&source)
        .set_name(format!("@{name}"))
        .eval()
        .map_err(|error| crate::error_text(&error))?;

    let mut params = Params::new();
    for pair in table.pairs::<String, Value>() {
        let (key, value) = pair.map_err(|error| format!("{name}: {error}"))?;
        let value = param_value(&value)
            .ok_or_else(|| format!("{name}: {key} must be a number or a pair like {{ 1, 2 }}"))?;
        params.insert(key, value);
    }
    Ok(params)
}

fn param_value(value: &Value) -> Option<ParamValue> {
    let number = |value: &Value| match value {
        Value::Number(number) => Some(*number),
        Value::Integer(number) => Some(*number as f64),
        _ => None,
    };
    match value {
        Value::Table(pair) => Some(ParamValue::Vec(
            number(&pair.raw_get(1).ok()?)?,
            number(&pair.raw_get(2).ok()?)?,
        )),
        other => number(other).map(ParamValue::Number),
    }
}

pub fn write_params(path: &Path, params: &Params) -> Result<(), String> {
    let mut text = String::from("return {\n");
    for (name, value) in params {
        let value = match value {
            ParamValue::Number(number) => number.to_string(),
            ParamValue::Vec(x, y) => format!("{{ {x}, {y} }}"),
        };
        text.push_str(&format!("  [{name:?}] = {value},\n"));
    }
    text.push_str("}\n");
    std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("anicode-{}-{name}", std::process::id()))
    }

    #[test]
    fn params_file_sits_next_to_the_scene() {
        assert_eq!(
            params_path(Path::new("examples/dot.lua")),
            Path::new("examples/dot.params.lua")
        );
    }

    #[test]
    fn written_params_read_back() {
        let path = temp_file("roundtrip.params.lua");
        let params = Params::from([
            ("dot.spin".to_string(), ParamValue::Vec(-1.0, 0.25)),
            ("title.size".to_string(), ParamValue::Number(0.8)),
        ]);
        write_params(&path, &params).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "return {\n  [\"dot.spin\"] = { -1, 0.25 },\n  [\"title.size\"] = 0.8,\n}\n"
        );
        assert_eq!(read_params(&path).unwrap(), params);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn bad_values_name_the_param() {
        let path = temp_file("bad.params.lua");
        std::fs::write(&path, "return { size = \"big\" }").unwrap();
        let error = read_params(&path).unwrap_err();
        assert!(
            error.ends_with("size must be a number or a pair like { 1, 2 }"),
            "{error}"
        );
        std::fs::remove_file(&path).unwrap();
    }
}
