//! Settings from config/anicode.toml.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{Engine, Settings};

const CONFIG_FILE: &str = "config/anicode.toml";

#[derive(Deserialize)]
pub struct Config {
    /// The folder that holds config/anicode.toml.
    #[serde(skip)]
    pub root: PathBuf,
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
    pub default_animation_seconds: f64,
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
        let mut config: Config =
            toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        config.root = absolute
            .parent()
            .and_then(Path::parent)
            .unwrap_or(Path::new("/"))
            .to_path_buf();
        Ok(config)
    }

    /// Loads the config of the project a scene belongs to: the nearest
    /// config/anicode.toml in the scene's folder or any folder above it.
    pub fn find_for(scene: &Path) -> Result<Config, String> {
        let scene = std::path::absolute(scene).unwrap_or_else(|_| PathBuf::from(scene));
        let path = scene
            .ancestors()
            .skip(1)
            .map(|folder| folder.join(CONFIG_FILE))
            .find(|path| path.is_file())
            .ok_or_else(|| {
                format!(
                    "no {CONFIG_FILE} found in the folders above {}",
                    scene.display()
                )
            })?;
        Config::load(&path)
    }
}

/// Loads a scene timed by `markers` (block name to start time in seconds).
pub fn load_scene(
    path: &Path,
    config: &Config,
    markers: &HashMap<String, f64>,
) -> Result<Engine, String> {
    let settings = Settings {
        frame_height: config.render.frame_height,
        default_block_seconds: config.timing.default_block_seconds,
        default_animation_seconds: config.timing.default_animation_seconds,
    };
    Engine::load(path, markers, settings)
}
