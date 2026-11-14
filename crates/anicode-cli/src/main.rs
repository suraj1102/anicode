use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anicode_lua::{Config, load_scene};
use anicode_render::{CpuRenderer, GpuRenderer, Pixmap};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "anicode", about = "Code-first animation engine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render a scene to numbered PNG files.
    Render {
        scene: PathBuf,
        /// Folder for the PNG files.
        #[arg(long)]
        out: PathBuf,
        /// Frame range like 0..90 (end not included). Default: the whole scene.
        #[arg(long)]
        frames: Option<String>,
        /// Render on the GPU instead of the CPU.
        #[arg(long)]
        gpu: bool,
        #[arg(long, default_value = "config/anicode.toml")]
        config: PathBuf,
    },
}

enum Renderer {
    Cpu(Box<CpuRenderer>),
    Gpu(Box<GpuRenderer>),
}

fn main() -> ExitCode {
    let Command::Render {
        scene,
        out,
        frames,
        gpu,
        config,
    } = Cli::parse().command;
    match render(&scene, &out, frames.as_deref(), gpu, &config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn render(
    scene: &Path,
    out: &Path,
    frames: Option<&str>,
    gpu: bool,
    config: &Path,
) -> Result<(), String> {
    let config = Config::load(config)?;
    let (width, height) = (config.render.width, config.render.height);
    let fps = config.render.fps;
    let engine = load_scene(scene, &config)?;

    let frame_count = (engine.duration() * fps).round() as u32 + 1;
    let (first, end) = match frames {
        Some(range) => parse_range(range)?,
        None => (0, frame_count),
    };

    let mut renderer = if gpu {
        Renderer::Gpu(Box::new(GpuRenderer::new(
            u32::from(width),
            u32::from(height),
        )?))
    } else {
        Renderer::Cpu(Box::new(CpuRenderer::new(width, height)))
    };
    std::fs::create_dir_all(out).map_err(|error| format!("{}: {error}", out.display()))?;

    let mut scene_time = Duration::ZERO;
    let mut render_time = Duration::ZERO;
    for frame in first..end {
        let started = Instant::now();
        let items = engine.display_list(f64::from(frame) / fps, width.into(), height.into())?;
        let built = Instant::now();
        let background = config.render.background;
        let pixmap = match &mut renderer {
            Renderer::Cpu(cpu) => cpu.render(&items, background),
            Renderer::Gpu(gpu) => gpu.render(&items, background)?,
        };
        render_time += built.elapsed();
        scene_time += built - started;
        write_png(pixmap, &out.join(format!("frame_{frame:05}.png")))?;
    }

    let count = end.saturating_sub(first).max(1);
    println!(
        "{} frames at {width}x{height}: scene {:.2} ms, render {:.2} ms per frame ({})",
        end.saturating_sub(first),
        scene_time.as_secs_f64() * 1000.0 / f64::from(count),
        render_time.as_secs_f64() * 1000.0 / f64::from(count),
        if gpu { "GPU" } else { "CPU" },
    );
    Ok(())
}

fn parse_range(range: &str) -> Result<(u32, u32), String> {
    let invalid = || format!("frame range must look like 0..90, got {range}");
    let (first, end) = range.split_once("..").ok_or_else(invalid)?;
    let first = first.parse().map_err(|_| invalid())?;
    let end = end.parse().map_err(|_| invalid())?;
    Ok((first, end))
}

fn write_png(pixmap: Pixmap, path: &Path) -> Result<(), String> {
    let png = pixmap.into_png().map_err(|error| error.to_string())?;
    std::fs::write(path, png).map_err(|error| format!("{}: {error}", path.display()))
}
