use std::collections::HashMap;

/// An `at` or `play` block as the scene declares it.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// The marker name that can pin it. Unnamed `play` blocks are numbered.
    pub name: String,
    /// How long the block lasts before the next one, from `seconds = n`.
    pub seconds: Option<f64>,
}

/// Start and end time of a block, in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockTime {
    pub start: f64,
    pub end: f64,
}

/// Places blocks in the order the scene declares them. A block starts at
/// its marker if one is set, otherwise where the previous block ends. It
/// lasts its `seconds` if set, otherwise `default_seconds` or its longest
/// animation, whichever is longer, so the next block never cuts one short.
pub fn block_times(
    blocks: &[Block],
    longest_animations: &[f64],
    markers: &HashMap<String, f64>,
    default_seconds: f64,
) -> Vec<BlockTime> {
    let mut next_start = 0.0;
    blocks
        .iter()
        .zip(longest_animations)
        .map(|(block, &longest)| {
            let start = markers.get(&block.name).copied().unwrap_or(next_start);
            let length = block.seconds.unwrap_or(default_seconds.max(longest));
            next_start = start + length;
            BlockTime {
                start,
                end: next_start,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(name: &str, seconds: Option<f64>) -> Block {
        Block {
            name: name.to_string(),
            seconds,
        }
    }

    fn time(start: f64, end: f64) -> BlockTime {
        BlockTime { start, end }
    }

    #[test]
    fn unpinned_blocks_follow_each_other() {
        let blocks = [block("1", None), block("2", Some(3.0)), block("3", None)];
        let times = block_times(&blocks, &[0.0, 0.0, 0.0], &HashMap::new(), 2.0);
        assert_eq!(times, [time(0.0, 2.0), time(2.0, 5.0), time(5.0, 7.0)]);
    }

    #[test]
    fn long_animations_extend_their_block() {
        let blocks = [block("1", None), block("2", None)];
        let times = block_times(&blocks, &[4.0, 1.0], &HashMap::new(), 2.0);
        assert_eq!(times, [time(0.0, 4.0), time(4.0, 6.0)]);
    }

    #[test]
    fn markers_pin_blocks_and_later_ones_follow() {
        let blocks = [block("intro", None), block("drop", None), block("3", None)];
        let markers = HashMap::from([("drop".to_string(), 10.0)]);
        let times = block_times(&blocks, &[0.0; 3], &markers, 2.0);
        assert_eq!(times, [time(0.0, 2.0), time(10.0, 12.0), time(12.0, 14.0)]);
    }
}
