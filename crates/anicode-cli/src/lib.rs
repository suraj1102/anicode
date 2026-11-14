//! Shared by the `anicode` command and the golden-frame tests.

use std::collections::HashMap;
use std::path::Path;

use anicode_lua::{Engine, Settings};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub render: RenderConfig,
    pub timing: TimingConfig,
    pub golden: GoldenConfig,
}

#[derive(Deserialize)]
pub struct RenderConfig {
    pub width: u16,
    pub height: u16,
    pub fps: f64,
    pub background: [f32; 4],
    pub frame_height: f64,
}

#[derive(Deserialize)]
pub struct TimingConfig {
    pub default_block_seconds: f64,
}

#[derive(Deserialize)]
pub struct GoldenConfig {
    pub width: u16,
    pub height: u16,
    pub scenes: Vec<String>,
    pub points: Vec<f64>,
    pub tolerance: u8,
}

impl Config {
    pub fn load(path: &Path) -> Result<Config, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    }
}

/// Loads a scene with no markers set, so every block gets the default length.
pub fn load_scene(path: &Path, config: &Config) -> Result<Engine, String> {
    let settings = Settings {
        frame_height: config.render.frame_height,
        default_block_seconds: config.timing.default_block_seconds,
    };
    Engine::load(path, &HashMap::new(), settings)
}
