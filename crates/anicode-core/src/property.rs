use std::collections::HashMap;

use crate::easing::Easing;
use crate::timeline::BlockTime;

pub type PropertyId = usize;
/// Index of a derive function. The Lua side owns the functions.
pub type DeriveId = usize;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    To(f64),
    By(f64),
    Times(f64),
}

impl Target {
    /// The value this target reaches when starting from `from`.
    pub fn apply(self, from: f64) -> f64 {
        match self {
            Target::To(value) => value,
            Target::By(offset) => from + offset,
            Target::Times(factor) => from * factor,
        }
    }
}

/// An animation as written in the scene. Its times are known only after all
/// blocks are declared, so it refers to its block by index.
#[derive(Clone, Copy, Debug)]
pub struct Animation {
    pub block: usize,
    /// `None` uses the default animation length.
    pub duration: Option<f64>,
    pub target: Target,
    pub easing: Easing,
}

#[derive(Clone, Copy, Debug)]
struct TimedAnimation {
    start: f64,
    duration: f64,
    from: f64,
    to: f64,
    easing: Easing,
}

impl TimedAnimation {
    fn value_at(&self, time: f64) -> f64 {
        let progress = if self.duration > 0.0 {
            ((time - self.start) / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.from + (self.to - self.from) * self.easing.apply(progress)
    }
}

#[derive(Debug, Default)]
struct Property {
    base: f64,
    derive: Option<DeriveId>,
    animations: Vec<Animation>,
    timed: Vec<TimedAnimation>,
}

/// Every animatable number in a scene. A value at time `t` depends only on
/// `t`, so any frame can be computed without playing the frames before it.
#[derive(Debug, Default)]
pub struct Properties {
    list: Vec<Property>,
}

impl Properties {
    pub fn add(&mut self, base: f64) -> PropertyId {
        self.list.push(Property {
            base,
            ..Default::default()
        });
        self.list.len() - 1
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn base(&self, id: PropertyId) -> f64 {
        self.list[id].base
    }

    pub fn set_base(&mut self, id: PropertyId, value: f64) {
        self.list[id].base = value;
    }

    pub fn derive(&self, id: PropertyId) -> Option<DeriveId> {
        self.list[id].derive
    }

    pub fn set_derive(&mut self, id: PropertyId, derive: DeriveId) {
        self.list[id].derive = Some(derive);
    }

    /// Returns the animation's index on this property.
    pub fn animate(&mut self, id: PropertyId, animation: Animation) -> usize {
        self.list[id].animations.push(animation);
        self.list[id].animations.len() - 1
    }

    pub fn animation_mut(&mut self, id: PropertyId, index: usize) -> &mut Animation {
        &mut self.list[id].animations[index]
    }

    /// The block of the first animation on a derived property, if any. A
    /// derived value ignores animations, so the scene has a mistake there.
    pub fn animated_derive_block(&self) -> Option<usize> {
        self.list
            .iter()
            .filter(|property| property.derive.is_some())
            .find_map(|property| property.animations.first())
            .map(|animation| animation.block)
    }

    /// Converts block-relative animations into absolute times. Each animation
    /// starts from the value the property has when it begins.
    pub fn resolve(&mut self, blocks: &[BlockTime], default_duration: f64) {
        for property in &mut self.list {
            let mut animations = property.animations.clone();
            animations.sort_by(|a, b| blocks[a.block].start.total_cmp(&blocks[b.block].start));

            property.timed.clear();
            for animation in animations {
                let block = blocks[animation.block];
                let start = block.start;
                let from = value_at(property.base, &property.timed, start);
                property.timed.push(TimedAnimation {
                    start,
                    duration: animation.duration.unwrap_or(default_duration),
                    from,
                    to: animation.target.apply(from),
                    easing: animation.easing,
                });
            }
        }
    }

    /// When the last animation finishes, in seconds.
    pub fn last_animation_end(&self) -> f64 {
        self.list
            .iter()
            .flat_map(|property| &property.timed)
            .map(|animation| animation.start + animation.duration)
            .fold(0.0, f64::max)
    }

    /// The animated value at `time`. Ignores derive; the caller handles that.
    pub fn value_at(&self, id: PropertyId, time: f64) -> f64 {
        let property = &self.list[id];
        value_at(property.base, &property.timed, time)
    }
}

fn value_at(base: f64, timed: &[TimedAnimation], time: f64) -> f64 {
    timed
        .iter()
        .rev()
        .find(|animation| time >= animation.start)
        .map_or(base, |animation| animation.value_at(time))
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Memo {
    InProgress,
    Done(f64),
}

/// Derived values computed so far for one frame. Tracks values being computed
/// so a derive that depends on itself is reported instead of looping forever.
#[derive(Debug)]
pub struct Frame {
    pub time: f64,
    memo: HashMap<PropertyId, Memo>,
}

pub struct CycleError;

impl Frame {
    pub fn new(time: f64) -> Frame {
        Frame {
            time,
            memo: HashMap::new(),
        }
    }

    pub fn cached(&self, id: PropertyId) -> Option<f64> {
        match self.memo.get(&id) {
            Some(Memo::Done(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn start(&mut self, id: PropertyId) -> Result<(), CycleError> {
        if self.memo.get(&id) == Some(&Memo::InProgress) {
            return Err(CycleError);
        }
        self.memo.insert(id, Memo::InProgress);
        Ok(())
    }

    pub fn finish(&mut self, id: PropertyId, value: f64) {
        self.memo.insert(id, Memo::Done(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animation(block: usize, target: Target) -> Animation {
        Animation {
            block,
            duration: None,
            target,
            easing: Easing::Linear,
        }
    }

    const BLOCKS: [BlockTime; 2] = [
        BlockTime {
            start: 0.0,
            end: 2.0,
        },
        BlockTime {
            start: 2.0,
            end: 4.0,
        },
    ];

    #[test]
    fn value_follows_animation_and_holds_after() {
        let mut properties = Properties::default();
        let x = properties.add(0.0);
        properties.animate(x, animation(0, Target::To(10.0)));
        properties.resolve(&BLOCKS, 2.0);
        assert_eq!(properties.value_at(x, 0.0), 0.0);
        assert_eq!(properties.value_at(x, 1.0), 5.0);
        assert_eq!(properties.value_at(x, 3.0), 10.0);
    }

    #[test]
    fn later_animation_starts_from_earlier_result() {
        let mut properties = Properties::default();
        let x = properties.add(1.0);
        properties.animate(x, animation(1, Target::Times(3.0)));
        properties.animate(x, animation(0, Target::By(1.0)));
        properties.resolve(&BLOCKS, 2.0);
        assert_eq!(properties.value_at(x, 2.0), 2.0);
        assert_eq!(properties.value_at(x, 4.0), 6.0);
    }

    #[test]
    fn duration_shortens_animation() {
        let mut properties = Properties::default();
        let x = properties.add(0.0);
        let index = properties.animate(x, animation(0, Target::To(1.0)));
        properties.animation_mut(x, index).duration = Some(0.5);
        properties.resolve(&BLOCKS, 2.0);
        assert_eq!(properties.value_at(x, 0.5), 1.0);
    }

    #[test]
    fn blocks_at_the_same_time_still_animate() {
        let mut properties = Properties::default();
        let x = properties.add(0.0);
        let y = properties.add(0.0);
        properties.animate(x, animation(0, Target::To(1.0)));
        properties.animate(y, animation(1, Target::To(1.0)));
        let stacked = [BlockTime {
            start: 3.0,
            end: 3.0,
        }; 2];
        properties.resolve(&stacked, 1.0);
        assert_eq!(properties.value_at(x, 3.5), 0.5);
        assert_eq!(properties.value_at(y, 3.5), 0.5);
        assert_eq!(properties.last_animation_end(), 4.0);
    }

    #[test]
    fn frame_reports_cycles() {
        let mut frame = Frame::new(0.0);
        assert!(frame.start(0).is_ok());
        assert!(frame.start(0).is_err());
        frame.finish(0, 4.0);
        assert_eq!(frame.cached(0), Some(4.0));
    }
}
