//! Reads and writes markers in saved Shotcut projects, so that
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
            // Shotcut writes a short-lived copy of the project next to it
            // while saving. It looks newest but is gone a moment later.
            if entry.file_name().to_string_lossy().starts_with("shotcut-") {
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

/// A clip of a scene on a project's timeline.
#[derive(Debug, PartialEq)]
pub struct SceneClip {
    pub scene: PathBuf,
    /// Where the clip starts on the timeline, in seconds.
    pub start: f64,
    /// How much of the scene's start the clip trims off, in seconds.
    pub in_point: f64,
}

/// The first clip of each scene on the project's timeline, which is the
/// clip the plugin times by.
pub fn scene_clips(project: &Path) -> Result<Vec<SceneClip>, String> {
    let xml = std::fs::read_to_string(project)
        .map_err(|error| format!("{}: {error}", project.display()))?;
    let project_dir = project.parent().unwrap_or(Path::new("/"));
    clips_from_xml(&xml, project_dir).map_err(|error| format!("{}: {error}", project.display()))
}

fn clips_from_xml(xml: &str, project_dir: &Path) -> Result<Vec<SceneClip>, String> {
    let document = Document::parse(xml).map_err(|error| error.to_string())?;
    let fps = frame_rate(&document)?;
    let scenes: HashMap<&str, PathBuf> = document
        .descendants()
        .filter(|node| node.has_tag_name("producer") || node.has_tag_name("chain"))
        .filter_map(|node| Some((node.attribute("id")?, scene_of(node, project_dir, true)?)))
        .collect();

    let mut clips: Vec<SceneClip> = vec![];
    for (id, start, in_point) in timeline_entries(&document, fps)? {
        let Some(scene) = scenes.get(id) else {
            continue;
        };
        if clips.iter().all(|clip| &clip.scene != scene) {
            clips.push(SceneClip {
                scene: scene.clone(),
                start,
                in_point,
            });
        }
    }
    Ok(clips)
}

/// Adds point markers to a project, written the way Shotcut writes them.
/// `markers` holds names and timeline times in seconds.
pub fn add_markers(xml: &str, markers: &[(String, f64)]) -> Result<String, String> {
    let document = Document::parse(xml).map_err(|error| error.to_string())?;
    let fps = frame_rate(&document)?;
    let entry = |index: usize, name: &str, time: f64| {
        let time = clock_time(time, fps);
        format!(
            "      <properties name=\"{index}\">\n\
             \x20       <property name=\"text\">{}</property>\n\
             \x20       <property name=\"start\">{time}</property>\n\
             \x20       <property name=\"end\">{time}</property>\n\
             \x20       <property name=\"color\">#008000</property>\n\
             \x20     </properties>\n",
            xml_escape(name)
        )
    };

    let list = document.descendants().find(|node| {
        node.has_tag_name("properties") && node.attribute("name") == Some("shotcut:markers")
    });
    let (insert_at, text) = match list {
        Some(list) => {
            let next_index = list
                .children()
                .filter_map(|child| child.attribute("name")?.parse::<usize>().ok())
                .map(|index| index + 1)
                .max()
                .unwrap_or(0);
            let entries: String = markers
                .iter()
                .enumerate()
                .map(|(i, (name, time))| entry(next_index + i, name, *time))
                .collect();
            // Before the list's closing tag, keeping its indentation.
            let close = list.range().end - "</properties>".len();
            (close, format!("{}    ", &entries[4..]))
        }
        None => {
            let tractor = document
                .descendants()
                .filter(|node| node.has_tag_name("tractor"))
                .find(|node| property(*node, "shotcut") == Some("1"))
                .ok_or("the project has no Shotcut timeline")?;
            let after_properties = tractor
                .children()
                .rfind(|child| child.has_tag_name("property"))
                .map(|child| child.range().end)
                .ok_or("the timeline has no properties")?;
            let entries: String = markers
                .iter()
                .enumerate()
                .map(|(i, (name, time))| entry(i, name, *time))
                .collect();
            let list =
                format!("\n    <properties name=\"shotcut:markers\">\n{entries}    </properties>");
            (after_properties, list)
        }
    };
    Ok(format!("{}{text}{}", &xml[..insert_at], &xml[insert_at..]))
}

/// Shotcut's clock format, snapped to a frame: "00:01:02.500".
fn clock_time(seconds: f64, fps: f64) -> String {
    let seconds = (seconds.max(0.0) * fps).round() / fps;
    let hours = (seconds / 3600.0).floor();
    let minutes = ((seconds - hours * 3600.0) / 60.0).floor();
    let rest = seconds - hours * 3600.0 - minutes * 60.0;
    format!("{hours:02}:{minutes:02}:{rest:06.3}")
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn is_project_with_scene(xml: &str, project_dir: &Path, scene: &Path) -> bool {
    // Wrappers written by `anicode new` name the scene too, but have no
    // timeline (tractor).
    if !xml.contains("<tractor") {
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

/// Ids of the producers and chains that render `scene`.
fn scene_producer_ids<'a>(
    document: &'a Document,
    project_dir: &Path,
    scene: &Path,
) -> Vec<&'a str> {
    let scene = normalize(scene);
    document
        .descendants()
        .filter(|node| node.has_tag_name("producer") || node.has_tag_name("chain"))
        .filter(|node| renders_scene(*node, project_dir, &scene, true))
        .filter_map(|node| node.attribute("id"))
        .collect()
}

fn renders_scene(node: Node, dir: &Path, scene: &Path, follow_wrappers: bool) -> bool {
    scene_of(node, dir, follow_wrappers).is_some_and(|path| path == scene)
}

/// The scene file a producer renders, if it is an anicode clip.
fn scene_of(node: Node, dir: &Path, follow_wrappers: bool) -> Option<PathBuf> {
    let path = normalize(&dir.join(property(node, "resource")?));
    match property(node, "mlt_service")? {
        "anicode" => Some(path),
        // A wrapper dragged into a project becomes a virtual clip that
        // points at the wrapper file, so look inside it.
        "xml-clip" | "xml" if follow_wrappers => {
            let wrapper_dir = path.parent().unwrap_or(Path::new("/"));
            let xml = std::fs::read_to_string(&path).ok()?;
            let wrapper = Document::parse(&xml).ok()?;
            wrapper
                .descendants()
                .find_map(|node| scene_of(node, wrapper_dir, false))
        }
        _ => None,
    }
}

/// Where the first clip of the scene starts on the timeline, and its
/// in-point, both in seconds. `None` if the scene is not on the timeline.
fn clip_placement(
    document: &Document,
    ids: &[&str],
    fps: f64,
) -> Result<Option<(f64, f64)>, String> {
    Ok(timeline_entries(document, fps)?
        .into_iter()
        .find(|(id, _, _)| ids.contains(id))
        .map(|(_, start, in_point)| (start, in_point)))
}

/// Every clip on every track: producer id, timeline start and in-point, in
/// seconds.
fn timeline_entries<'a>(
    document: &'a Document,
    fps: f64,
) -> Result<Vec<(&'a str, f64, f64)>, String> {
    let mut entries = vec![];
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
                if let Some(id) = item.attribute("producer") {
                    entries.push((id, position, in_point));
                }
                // The out point is the last frame shown, so add one frame.
                position += out_point - in_point + 1.0 / fps;
            }
        }
    }
    Ok(entries)
}

/// MLT accepts a property as a child element or as an attribute.
fn property<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    if let Some(value) = node.attribute(name) {
        return Some(value);
    }
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
    fn follows_virtual_clips_to_their_wrapper() {
        let root = std::env::temp_dir().join(format!("anicode-virtual-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("shape.mlt"),
            r#"<mlt><producer id="shape" mlt_service="anicode"><property name="resource">shape.lua</property></producer></mlt>"#,
        )
        .unwrap();
        // How Shotcut 26.9 saves a wrapper dragged onto the timeline.
        let project = PROJECT
            .replace(
                r#"<property name="resource">shape.lua</property>"#,
                r#"<property name="resource">shape.mlt</property>"#,
            )
            .replace(
                r#"<property name="mlt_service">anicode</property>"#,
                r#"<property name="mlt_service">xml-clip</property>"#,
            );

        let markers = markers_from_xml(&project, &root, &root.join("shape.lua"));
        std::fs::remove_dir_all(&root).unwrap();
        assert!((markers.unwrap()["move"] - 2.88).abs() < 1e-9);
    }

    #[test]
    fn lists_scene_clips_on_the_timeline() {
        let clips = clips_from_xml(PROJECT, Path::new("/p")).unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].scene, PathBuf::from("/p/shape.lua"));
        assert!((clips[0].start - 1.08).abs() < 1e-9 && clips[0].in_point == 0.0);
    }

    #[test]
    fn added_markers_read_back() {
        let markers = [("shape:3".to_string(), 6.08), ("shape:4".to_string(), 8.0)];
        let project = add_markers(PROJECT, &markers).unwrap();
        let read = markers_from_xml(&project, Path::new("/p"), Path::new("/p/shape.lua")).unwrap();
        // Existing markers stay; new ones are relative to the clip at 1.08 s.
        assert!((read["move"] - 2.88).abs() < 1e-9);
        assert!((read["3"] - 5.0).abs() < 1e-9);
        assert!((read["4"] - 6.92).abs() < 1e-9);
    }

    #[test]
    fn adds_a_marker_list_when_there_is_none() {
        let start = PROJECT
            .find("    <properties name=\"shotcut:markers\">")
            .unwrap();
        let end = PROJECT.find("    <track ").unwrap();
        let without = format!("{}{}", &PROJECT[..start], &PROJECT[end..]);
        let project = add_markers(&without, &[("shape:1".to_string(), 1.08)]).unwrap();
        let read = markers_from_xml(&project, Path::new("/p"), Path::new("/p/shape.lua")).unwrap();
        assert_eq!(read.len(), 1);
        assert!(read["1"].abs() < 1e-9);
    }

    #[test]
    fn clock_times_snap_to_frames() {
        assert_eq!(clock_time(62.48, 25.0), "00:01:02.480");
        assert_eq!(clock_time(4.77, 30000.0 / 1001.0), "00:00:04.771");
        assert_eq!(clock_time(3725.0, 25.0), "01:02:05.000");
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
        // Shotcut's temporary copy while saving: newer, but must be skipped.
        std::fs::write(
            root.join("episode/shotcut-AbC123.mlt"),
            PROJECT.replace("shape.lua", "../shape.lua"),
        )
        .unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(root.join("episode/shotcut-AbC123.mlt"))
            .unwrap()
            .set_modified(later)
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
