//! Undo and redo as a tree of params file states. Undoing and then making a
//! new change starts a branch; the old branch stays and can be jumped to.

use std::time::SystemTime;

use anicode_lua::Params;

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
    fn recording_the_same_params_adds_nothing() {
        let mut history = History::new(params(0.0));
        assert!(!history.record(params(0.0), "no change"));
        assert_eq!(history.nodes().len(), 1);
    }
}
