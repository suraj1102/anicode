//! Undo and redo as a tree of params file states. Undoing and then making a
//! new change starts a branch; the old branch stays and can be jumped to.

use std::path::Path;
use std::time::{Duration, SystemTime};

use anicode_lua::Params;
use serde::{Deserialize, Serialize};

pub struct HistoryNode {
    pub parent: Option<usize>,
    /// The newest child is last; redo follows it.
    pub children: Vec<usize>,
    pub params: Params,
    /// Like "drag dot.start" or "edited outside the editor".
    pub label: String,
    pub when: SystemTime,
}

pub struct History {
    nodes: Vec<HistoryNode>,
    current: usize,
}

impl History {
    /// Starts with the params file as it is when the editor opens.
    pub fn new(params: Params) -> History {
        History {
            nodes: vec![HistoryNode {
                parent: None,
                children: vec![],
                params,
                label: "opened".into(),
                when: SystemTime::now(),
            }],
            current: 0,
        }
    }

    pub fn nodes(&self) -> &[HistoryNode] {
        &self.nodes
    }

    pub fn current(&self) -> usize {
        self.current
    }

    pub fn params(&self) -> &Params {
        &self.nodes[self.current].params
    }

    /// Adds a state after the current one. Returns false, and adds nothing,
    /// when the params are the same as the current state's.
    pub fn record(&mut self, params: Params, label: impl Into<String>) -> bool {
        if params == *self.params() {
            return false;
        }
        self.nodes.push(HistoryNode {
            parent: Some(self.current),
            children: vec![],
            params,
            label: label.into(),
            when: SystemTime::now(),
        });
        let added = self.nodes.len() - 1;
        self.nodes[self.current].children.push(added);
        self.current = added;
        true
    }

    /// Moves to the parent state. Returns false at the first state.
    pub fn undo(&mut self) -> bool {
        match self.nodes[self.current].parent {
            Some(parent) => {
                self.current = parent;
                true
            }
            None => false,
        }
    }

    /// Moves to the newest child state. Returns false when there is none.
    pub fn redo(&mut self) -> bool {
        match self.nodes[self.current].children.last() {
            Some(&child) => {
                self.current = child;
                true
            }
            None => false,
        }
    }

    pub fn jump(&mut self, node: usize) {
        self.current = node;
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let saved = SavedHistory {
            current: self.current,
            nodes: self
                .nodes
                .iter()
                .map(|node| SavedNode {
                    parent: node.parent,
                    label: node.label.clone(),
                    when: node
                        .when
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map_or(0, |since| since.as_secs()),
                    params: node.params.clone(),
                })
                .collect(),
        };
        let text = toml::to_string(&saved).map_err(|error| error.to_string())?;
        let error = |error: std::io::Error| format!("{}: {error}", path.display());
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(error)?;
        }
        std::fs::write(path, text).map_err(error)
    }

    pub fn load(path: &Path) -> Result<History, String> {
        let error = |error: String| format!("{}: {error}", path.display());
        let text = std::fs::read_to_string(path).map_err(|e| error(e.to_string()))?;
        let saved: SavedHistory = toml::from_str(&text).map_err(|e| error(e.to_string()))?;
        let mut nodes: Vec<HistoryNode> = saved
            .nodes
            .into_iter()
            .map(|node| HistoryNode {
                parent: node.parent,
                children: vec![],
                params: node.params,
                label: node.label,
                when: SystemTime::UNIX_EPOCH + Duration::from_secs(node.when),
            })
            .collect();
        // Children come after their parent, so this keeps the newest last.
        for index in 0..nodes.len() {
            let parent = nodes[index].parent;
            match parent {
                Some(parent) if parent < index => nodes[parent].children.push(index),
                None if index == 0 => {}
                _ => return Err(error(format!("state {index} has a bad parent"))),
            }
        }
        if nodes.is_empty() || saved.current >= nodes.len() {
            return Err(error("the current state is missing".into()));
        }
        Ok(History {
            nodes,
            current: saved.current,
        })
    }
}

/// The history file: one entry per state, with times in seconds since
/// 1970. Children are not stored; they follow from the parents.
#[derive(Serialize, Deserialize)]
struct SavedHistory {
    current: usize,
    nodes: Vec<SavedNode>,
}

#[derive(Serialize, Deserialize)]
struct SavedNode {
    parent: Option<usize>,
    label: String,
    when: u64,
    params: Params,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anicode_lua::ParamValue;

    fn params(x: f64) -> Params {
        Params::from([("x".to_string(), ParamValue::Number(x))])
    }

    #[test]
    fn undo_and_redo_walk_the_states() {
        let mut history = History::new(params(0.0));
        history.record(params(1.0), "drag x");
        history.record(params(2.0), "drag x");
        assert!(history.undo());
        assert_eq!(*history.params(), params(1.0));
        assert!(history.undo());
        assert!(!history.undo());
        assert!(history.redo());
        assert!(history.redo());
        assert!(!history.redo());
        assert_eq!(*history.params(), params(2.0));
    }

    #[test]
    fn a_change_after_undo_starts_a_branch_that_redo_follows() {
        let mut history = History::new(params(0.0));
        history.record(params(1.0), "first");
        history.undo();
        history.record(params(5.0), "second");
        assert_eq!(history.nodes()[0].children, [1, 2]);
        history.undo();
        history.redo();
        assert_eq!(*history.params(), params(5.0));
        // The first branch is still there.
        history.jump(1);
        assert_eq!(*history.params(), params(1.0));
    }

    #[test]
    fn saved_history_loads_back_the_same() {
        let mut history = History::new(params(0.0));
        history.record(
            Params::from([("dot.end".to_string(), ParamValue::Vec(1.0, -2.5))]),
            "drag dot.end",
        );
        history.undo();
        history.record(params(3.0), "drag x");
        let path = std::env::temp_dir()
            .join(format!("anicode-history-{}", std::process::id()))
            .join("scene.toml");
        history.save(&path).unwrap();
        let loaded = History::load(&path).unwrap();
        assert_eq!(loaded.current(), history.current());
        assert_eq!(loaded.nodes()[0].children, [1, 2]);
        for (a, b) in loaded.nodes().iter().zip(history.nodes()) {
            assert_eq!(
                (&a.params, &a.label, a.parent),
                (&b.params, &b.label, b.parent)
            );
        }
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn recording_the_same_params_adds_nothing() {
        let mut history = History::new(params(0.0));
        assert!(!history.record(params(0.0), "no change"));
        assert_eq!(history.nodes().len(), 1);
    }
}
