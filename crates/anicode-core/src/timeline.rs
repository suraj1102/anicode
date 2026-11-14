use std::collections::HashMap;

/// Start and end time of an `at` block, in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockTime {
    pub start: f64,
    pub end: f64,
}

/// Places blocks in the order they appear in the scene. A block starts at its
/// marker if one is set, otherwise `default_seconds` after the previous block.
/// Each block ends where the next one starts; the last one runs for
/// `default_seconds`.
pub fn block_times(
    block_names: &[String],
    markers: &HashMap<String, f64>,
    default_seconds: f64,
) -> Vec<BlockTime> {
    let mut starts: Vec<f64> = Vec::with_capacity(block_names.len());
    for name in block_names {
        let start = match markers.get(name) {
            Some(&time) => time,
            None => starts
                .last()
                .map_or(0.0, |previous| previous + default_seconds),
        };
        starts.push(start);
    }

    starts
        .iter()
        .enumerate()
        .map(|(i, &start)| BlockTime {
            start,
            end: starts
                .get(i + 1)
                .copied()
                .unwrap_or(start + default_seconds),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn missing_markers_use_default_spacing() {
        let times = block_times(&names(&["a", "b"]), &HashMap::new(), 2.0);
        assert_eq!(
            times[0],
            BlockTime {
                start: 0.0,
                end: 2.0
            }
        );
        assert_eq!(
            times[1],
            BlockTime {
                start: 2.0,
                end: 4.0
            }
        );
    }

    #[test]
    fn markers_set_block_starts() {
        let markers = HashMap::from([("a".to_string(), 1.0), ("b".to_string(), 5.0)]);
        let times = block_times(&names(&["a", "b", "c"]), &markers, 2.0);
        assert_eq!(
            times[0],
            BlockTime {
                start: 1.0,
                end: 5.0
            }
        );
        assert_eq!(
            times[1],
            BlockTime {
                start: 5.0,
                end: 7.0
            }
        );
        assert_eq!(
            times[2],
            BlockTime {
                start: 7.0,
                end: 9.0
            }
        );
    }
}
