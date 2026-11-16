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
    parse_params(&source, &path.display().to_string())
}

fn parse_params(source: &str, name: &str) -> Result<Params, String> {
    let lua = Lua::new();
    let table: Table = lua
        .load(source)
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
        text.push_str(&param_line(name, *value));
        text.push('\n');
    }
    text.push_str("}\n");
    write(path, &text)
}

/// Sets these params in the file and keeps the rest of it as written,
/// comments included. Writes a new file if there is none.
pub fn set_params(path: &Path, changes: &Params) -> Result<(), String> {
    if !path.is_file() {
        return write_params(path, changes);
    }
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let name = path.display().to_string();
    let mut expected = parse_params(&text, &name)?;
    expected.extend(changes.iter().map(|(name, value)| (name.clone(), *value)));
    // The edit finds one param per line. For a file laid out another way,
    // the edit reads back wrong, and the file is written fresh instead.
    let edited = edit_params_text(&text, changes);
    match parse_params(&edited, &name) {
        Ok(params) if params == expected => write(path, &edited),
        _ => write_params(path, &expected),
    }
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

fn param_line(name: &str, value: ParamValue) -> String {
    format!("  [{name:?}] = {},", value_text(value))
}

fn value_text(value: ParamValue) -> String {
    match value {
        ParamValue::Number(number) => number.to_string(),
        ParamValue::Vec(x, y) => format!("{{ {x}, {y} }}"),
    }
}

/// Replaces the value on each changed param's line, and adds params the
/// file does not have before its closing brace.
fn edit_params_text(text: &str, changes: &Params) -> String {
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    for (name, value) in changes {
        let found = lines
            .iter()
            .enumerate()
            .find_map(|(index, line)| Some((index, value_start(line, name)?)));
        match found {
            Some((index, start)) => {
                let line = &lines[index];
                let end = value_end(line, start);
                lines[index] = format!("{}{}{}", &line[..start], value_text(*value), &line[end..]);
            }
            None => {
                let closing = lines
                    .iter()
                    .rposition(|line| line.trim_start().starts_with('}'))
                    .unwrap_or(lines.len());
                lines.insert(closing, param_line(name, *value));
            }
        }
    }
    let mut edited = lines.join("\n");
    if text.ends_with('\n') {
        edited.push('\n');
    }
    edited
}

/// Where the value starts on a line like `["dot.end"] = { 1, 2 },` or
/// `size = 0.8,`, if the line sets `name`.
fn value_start(line: &str, name: &str) -> Option<usize> {
    let rest = line.trim_start();
    let keys = [
        format!("[{name:?}]"),
        format!("['{name}']"),
        name.to_string(),
    ];
    let after_key = keys
        .iter()
        .find_map(|key| rest.strip_prefix(key.as_str()))?;
    let value = after_key.trim_start().strip_prefix('=')?.trim_start();
    Some(line.len() - value.len())
}

/// Where the value that starts at `start` ends: before the comma, closing
/// brace or comment that follows it.
fn value_end(line: &str, start: usize) -> usize {
    let mut depth = 0;
    let mut end = line.len();
    for (offset, character) in line[start..].char_indices() {
        let at = start + offset;
        match character {
            '{' => depth += 1,
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    end = at + 1;
                    break;
                }
            }
            ',' | '}' if depth == 0 => {
                end = at;
                break;
            }
            '-' if depth == 0 && line[at..].starts_with("--") => {
                end = at;
                break;
            }
            _ => {}
        }
    }
    start + line[start..end].trim_end().len()
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
    fn setting_params_keeps_comments_and_layout() {
        let path = temp_file("comments.params.lua");
        let text = "-- Timing for the intro.\nreturn {\n  size = 0.8, -- big\n  [\"dot.end\"] = { 1, 2 }, -- keep\n}\n";
        std::fs::write(&path, text).unwrap();
        let changes = Params::from([
            ("dot.end".to_string(), ParamValue::Vec(3.0, -1.5)),
            ("new".to_string(), ParamValue::Number(2.0)),
        ]);
        set_params(&path, &changes).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "-- Timing for the intro.\nreturn {\n  size = 0.8, -- big\n  [\"dot.end\"] = { 3, -1.5 }, -- keep\n  [\"new\"] = 2,\n}\n"
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn setting_params_in_an_unusual_file_rewrites_it() {
        let path = temp_file("unusual.params.lua");
        std::fs::write(&path, "local t = {}\nt.size = 0.8\nreturn t\n").unwrap();
        let changes = Params::from([("wait".to_string(), ParamValue::Number(3.0))]);
        set_params(&path, &changes).unwrap();
        let expected = Params::from([
            ("size".to_string(), ParamValue::Number(0.8)),
            ("wait".to_string(), ParamValue::Number(3.0)),
        ]);
        assert_eq!(read_params(&path).unwrap(), expected);
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
