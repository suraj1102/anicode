//! Reads marker times for a scene from a saved Shotcut project, so that
//! moving a marker in Shotcut re-times the scene.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use roxmltree::{Document, Node};

/// Folders that hold build output, not projects.
const SKIPPED_FOLDERS: [&str; 4] = ["target", "build", ".git", "node_modules"];

/// The most recently saved Shotcut project under `root` that has a clip of
/// `scene` on its timeline.
pub fn find_project(root: &Path, scene: &Path) -> Option<PathBuf> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !SKIPPED_FOLDERS.contains(&&*entry.file_name().to_string_lossy()) {
                    folders.push(path);
                }
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "mlt") {
                continue;
            }
            let Ok(xml) = std::fs::read_to_string(&path) else {
                continue;
            };
            let project_dir = path.parent().unwrap_or(Path::new("/"));
            if !is_project_with_scene(&xml, project_dir, scene) {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
                newest = Some((modified, path));
            }
        }
    }
    newest.map(|(_, path)| path)
}

/// Marker times in seconds from the start of the scene's clip. A marker named
/// `scene:block` wins over one named just `block`.
pub fn clip_markers(project: &Path, scene: &Path) -> Result<HashMap<String, f64>, String> {
    let xml = std::fs::read_to_string(project)
        .map_err(|error| format!("{}: {error}", project.display()))?;
    let project_dir = project.parent().unwrap_or(Path::new("/"));
    markers_from_xml(&xml, project_dir, scene)
        .map_err(|error| format!("{}: {error}", project.display()))
}

fn is_project_with_scene(xml: &str, project_dir: &Path, scene: &Path) -> bool {
    // Cheap checks first. Wrappers written by `anicode new` name the scene
    // too, but have no timeline (tractor).
    if !xml.contains("anicode") || !xml.contains("<tractor") {
        return false;
    }
    Document::parse(xml)
        .is_ok_and(|document| !scene_producer_ids(&document, project_dir, scene).is_empty())
}

fn markers_from_xml(
    xml: &str,
    project_dir: &Path,
    scene: &Path,
) -> Result<HashMap<String, f64>, String> {
    let document = Document::parse(xml).map_err(|error| error.to_string())?;
    let fps = frame_rate(&document)?;
    let ids = scene_producer_ids(&document, project_dir, scene);
    let Some((clip_start, in_point)) = clip_placement(&document, &ids, fps)? else {
        return Ok(HashMap::new());
    };

    let prefix = format!(
        "{}:",
        scene.file_stem().unwrap_or_default().to_string_lossy()
    );
    let mut markers = HashMap::new();
    let mut scene_markers = HashMap::new();
    let marker_lists = document.descendants().filter(|node| {
        node.has_tag_name("properties") && node.attribute("name") == Some("shotcut:markers")
    });
    for marker in marker_lists.flat_map(|list| list.children().filter(Node::is_element)) {
        let (Some(text), Some(start)) = (property(marker, "text"), property(marker, "start"))
        else {
            continue;
        };
        let time = parse_time(start, fps)? - clip_start + in_point;
        match text.strip_prefix(&prefix) {
            Some(name) => scene_markers.insert(name.to_string(), time),
            None => markers.insert(text.to_string(), time),
        };
    }
    markers.extend(scene_markers);
    Ok(markers)
}

fn frame_rate(document: &Document) -> Result<f64, String> {
    let profile = document
        .descendants()
        .find(|node| node.has_tag_name("profile"))
        .ok_or("the project has no profile")?;
    let number = |name: &str| -> Result<f64, String> {
        profile
            .attribute(name)
            .and_then(|value| value.parse().ok())
            .ok_or(format!("the profile has no valid {name}"))
    };
    Ok(number("frame_rate_num")? / number("frame_rate_den")?)
}

/// Ids of the producers that render `scene` with the anicode service.
fn scene_producer_ids<'a>(
    document: &'a Document,
    project_dir: &Path,
    scene: &Path,
) -> Vec<&'a str> {
    let scene = normalize(scene);
    document
        .descendants()
        .filter(|node| node.has_tag_name("producer") || node.has_tag_name("chain"))
        .filter(|node| property(*node, "mlt_service") == Some("anicode"))
        .filter(|node| {
            property(*node, "resource")
                .is_some_and(|resource| normalize(&project_dir.join(resource)) == scene)
        })
        .filter_map(|node| node.attribute("id"))
        .collect()
}

/// Where the first clip of the scene starts on the timeline, and its
/// in-point, both in seconds. `None` if the scene is not on the timeline.
fn clip_placement(
    document: &Document,
    ids: &[&str],
    fps: f64,
) -> Result<Option<(f64, f64)>, String> {
    for playlist in document
        .descendants()
        .filter(|node| node.has_tag_name("playlist"))
    {
        let mut position = 0.0;
        for item in playlist.children().filter(Node::is_element) {
            let time = |name: &str| parse_time(item.attribute(name).unwrap_or("0"), fps);
            if item.has_tag_name("blank") {
                position += time("length")?;
            } else if item.has_tag_name("entry") {
                let (in_point, out_point) = (time("in")?, time("out")?);
                if item
                    .attribute("producer")
                    .is_some_and(|id| ids.contains(&id))
                {
                    return Ok(Some((position, in_point)));
                }
                // The out point is the last frame shown, so add one frame.
                position += out_point - in_point + 1.0 / fps;
            }
        }
    }
    Ok(None)
}

fn property<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.children()
        .find(|child| child.has_tag_name("property") && child.attribute("name") == Some(name))
        .and_then(|child| child.text())
}

/// Accepts Shotcut's clock times ("00:00:01.080") and plain frame numbers.
fn parse_time(text: &str, fps: f64) -> Result<f64, String> {
    let invalid = || format!("invalid time \"{text}\"");
    if !text.contains(':') {
        return text
            .parse::<f64>()
            .map(|frames| frames / fps)
            .map_err(|_| invalid());
    }
    text.split(':').try_fold(0.0, |total, part| {
        part.parse::<f64>()
            .map(|value| total * 60.0 + value)
            .map_err(|_| invalid())
    })
}

/// Makes a path absolute and removes `.` and `..` without touching the disk.
fn normalize(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut result = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Saved by Shotcut 26.9.28: a clip of shape.lua placed at 1.08 s, with
    /// markers "intro" at 1.08 s and "move" at 3.96 s.
    const PROJECT: &str = include_str!("../tests/fixtures/shotcut-26.9.mlt");

    #[test]
    fn markers_become_clip_times() {
        let markers =
            markers_from_xml(PROJECT, Path::new("/p"), Path::new("/p/shape.lua")).unwrap();
        assert!((markers["intro"] - 0.0).abs() < 1e-9);
        assert!((markers["move"] - 2.88).abs() < 1e-9);
    }

    #[test]
    fn other_scenes_get_no_markers() {
        let markers =
            markers_from_xml(PROJECT, Path::new("/p"), Path::new("/p/other.lua")).unwrap();
        assert!(markers.is_empty());
    }

    #[test]
    fn scene_markers_win_and_in_point_shifts_times() {
        let project = PROJECT.replace(">move<", ">shape:intro<").replace(
            r#"<entry producer="shape" in="00:00:00.000""#,
            r#"<entry producer="shape" in="00:00:00.500""#,
        );
        let markers =
            markers_from_xml(&project, Path::new("/p"), Path::new("/p/shape.lua")).unwrap();
        // "shape:intro" at 3.96 s beats "intro" at 1.08 s; clip starts at 1.08 s, trimmed by 0.5 s.
        assert!((markers["intro"] - 3.38).abs() < 1e-9);
    }

    #[test]
    fn parses_clock_and_frame_times() {
        assert_eq!(parse_time("00:01:02.500", 25.0).unwrap(), 62.5);
        assert_eq!(parse_time("50", 25.0).unwrap(), 2.0);
        assert!(parse_time("1:x", 25.0).is_err());
    }

    #[test]
    fn finds_the_project_that_uses_the_scene() {
        let root = std::env::temp_dir().join(format!("anicode-find-{}", std::process::id()));
        std::fs::create_dir_all(root.join("episode")).unwrap();
        std::fs::write(
            root.join("episode/edit.mlt"),
            PROJECT.replace("shape.lua", "../shape.lua"),
        )
        .unwrap();
        // A wrapper mentions the scene but is not a project.
        std::fs::write(
            root.join("shape.mlt"),
            r#"<mlt><producer id="shape" mlt_service="anicode"><property name="resource">shape.lua</property></producer></mlt>"#,
        )
        .unwrap();

        let found = find_project(&root, &root.join("shape.lua"));
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(found, Some(root.join("episode/edit.mlt")));
    }
}
