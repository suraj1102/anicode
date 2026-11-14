//! Renders example scenes on the CPU and compares them to reference PNGs in
//! tests/golden. To accept new output, run with ANICODE_UPDATE_GOLDEN=1.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anicode_cli::{Config, load_scene};
use anicode_render::{CpuRenderer, Pixmap};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn examples_match_golden_frames() {
    let root = repo_root();
    let config = Config::load(&root.join("config/anicode.toml")).unwrap();
    let golden = &config.golden;
    let update = std::env::var_os("ANICODE_UPDATE_GOLDEN").is_some();
    let failures_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("golden-failures");
    let mut failures = vec![];

    for scene_name in &golden.scenes {
        let engine = load_scene(&root.join(format!("examples/{scene_name}.lua")), &config).unwrap();
        let mut renderer = CpuRenderer::new(golden.width, golden.height);

        for (index, point) in golden.points.iter().enumerate() {
            let time = point * engine.duration();
            let items = engine
                .display_list(time, golden.width.into(), golden.height.into())
                .unwrap();
            let actual = renderer.render(&items, config.render.background);
            let file_name = format!("{scene_name}_{index}.png");
            let golden_path = root.join("tests/golden").join(&file_name);

            if update {
                std::fs::write(&golden_path, actual.into_png().unwrap()).unwrap();
                continue;
            }
            let expected =
                Pixmap::from_png(BufReader::new(File::open(&golden_path).unwrap())).unwrap();
            let difference = largest_difference(&actual, &expected);
            if difference > golden.tolerance {
                std::fs::create_dir_all(&failures_dir).unwrap();
                let actual_path = failures_dir.join(&file_name);
                std::fs::write(&actual_path, actual.into_png().unwrap()).unwrap();
                failures.push(format!(
                    "{file_name}: differs by {difference}, output written to {}",
                    actual_path.display()
                ));
            }
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn largest_difference(a: &Pixmap, b: &Pixmap) -> u8 {
    assert_eq!(
        (a.width(), a.height()),
        (b.width(), b.height()),
        "frame size changed"
    );
    a.data_as_u8_slice()
        .iter()
        .zip(b.data_as_u8_slice())
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}
