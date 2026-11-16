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

/// Where a value came from: a `param("name", default)` in the scene. `param`
/// indexes the params the scene used, in the order it first used them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamUse {
    pub param: usize,
    pub part: ParamPart,
    /// The value is a number plus `gain` times the param: 2 for `p * 2`.
    pub gain: f64,
    /// The node whose position the value was read from, like `dot` in
    /// `dot:pos() + vec(1, 0)`. Dragging this value would move that node.
    pub via: Option<usize>,
}

impl ParamUse {
    pub fn scaled(self, factor: f64) -> ParamUse {
        ParamUse {
            gain: self.gain * factor,
            ..self
        }
    }
}

/// Which part of the param the value is: all of a number param, or one
/// coordinate of a vec param.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamPart {
    Number,
    X,
    Y,
}

/// What sets a property's value at some time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Control {
    /// Its gain says how much the value changes, at that time, when the
    /// param changes by 1. Part way through an animation to a param, that is
    /// the eased progress.
    Param(ParamUse),
    Derive,
    /// A number written in the code: the base value (`block` is `None`) or
    /// the target of an animation in that block.
    Code {
        block: Option<usize>,
    },
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
    /// The param the target value came from, if any.
    pub param: Option<ParamUse>,
}

#[derive(Clone, Copy, Debug)]
struct TimedAnimation {
    /// Index of the animation this was made from.
    source: usize,
    start: f64,
    duration: f64,
    from: f64,
    to: f64,
    easing: Easing,
}

impl TimedAnimation {
    /// How far the value has gone from `from` to `to`, 0 to 1, after easing.
    fn eased_progress(&self, time: f64) -> f64 {
        let progress = if self.duration > 0.0 {
            ((time - self.start) / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.easing.apply(progress)
    }

    fn value_at(&self, time: f64) -> f64 {
        self.from + (self.to - self.from) * self.eased_progress(time)
    }
}

#[derive(Debug, Default)]
struct Property {
    base: f64,
    /// The param behind the base value.
    base_param: Option<ParamUse>,
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
        self.list[id].base_param = None;
    }

    /// Changes the base value and keeps track of the param it came from.
    /// After a relative change by a number in code, the result still follows
    /// the earlier param, so dragging that param moves it the same way.
    pub fn change_base(&mut self, id: PropertyId, target: Target, param: Option<ParamUse>) {
        let property = &mut self.list[id];
        let old = property.base;
        property.base = target.apply(old);
        property.base_param = match (target, property.base_param) {
            (Target::By(_), Some(earlier)) => Some(earlier),
            (Target::Times(factor), Some(earlier)) => Some(earlier.scaled(factor)),
            // Scaling by a param multiplies it by the old value.
            (Target::Times(_), None) => param.map(|param| param.scaled(old)),
            (Target::To(_) | Target::By(_), _) => param,
        };
    }

    /// The param behind the base value, if any.
    pub fn base_param(&self, id: PropertyId) -> Option<ParamUse> {
        self.list[id].base_param
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
            let mut order: Vec<usize> = (0..property.animations.len()).collect();
            order.sort_by(|&a, &b| {
                let start = |index: usize| blocks[property.animations[index].block].start;
                start(a).total_cmp(&start(b))
            });

            property.timed.clear();
            for source in order {
                let animation = property.animations[source];
                let start = blocks[animation.block].start;
                let from = value_at(property.base, &property.timed, start);
                property.timed.push(TimedAnimation {
                    source,
                    start,
                    duration: animation.duration.unwrap_or(default_duration),
                    from,
                    to: animation.target.apply(from),
                    easing: animation.easing,
                });
            }
        }
    }

    /// The length of the longest animation in each block.
    pub fn longest_animations(&self, block_count: usize, default_duration: f64) -> Vec<f64> {
        let mut longest = vec![0.0; block_count];
        for animation in self.list.iter().flat_map(|property| &property.animations) {
            let duration = animation.duration.unwrap_or(default_duration);
            longest[animation.block] = f64::max(longest[animation.block], duration);
        }
        longest
    }

    /// When the last animation finishes, in seconds.
    pub fn last_animation_end(&self) -> f64 {
        self.list
            .iter()
            .flat_map(|property| &property.timed)
            .map(|animation| animation.start + animation.duration)
            .fold(0.0, f64::max)
    }

    /// What sets the value at `time`: the animation that last started, or the
    /// base value before any has. At its start time an animation still shows
    /// its start value, so it counts only after that. An animation by a relative number in code
    /// (`shift(dot, RIGHT)`) follows its start value, so the search goes on
    /// to what set that.
    pub fn control(&self, id: PropertyId, time: f64) -> Control {
        let property = &self.list[id];
        if property.derive.is_some() {
            return Control::Derive;
        }
        let started = property
            .timed
            .iter()
            .filter(|animation| time > animation.start)
            .count();
        // Each relative animation in code on the way back scales the gain.
        let mut gain = 1.0;
        for timed in property.timed[..started].iter().rev() {
            let animation = property.animations[timed.source];
            let progress = timed.eased_progress(time);
            match (animation.param, animation.target) {
                (Some(param), Target::To(_) | Target::By(_)) => {
                    return Control::Param(param.scaled(gain * progress));
                }
                (Some(param), Target::Times(_)) => {
                    return Control::Param(param.scaled(gain * timed.from * progress));
                }
                (None, Target::By(_)) => {}
                (None, Target::Times(factor)) => gain *= 1.0 + (factor - 1.0) * progress,
                (None, Target::To(_)) => {
                    return Control::Code {
                        block: Some(animation.block),
                    };
                }
            }
        }
        match property.base_param {
            Some(param) => Control::Param(param.scaled(gain)),
            None => Control::Code { block: None },
        }
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
            param: None,
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

    fn param(param: usize, part: ParamPart) -> ParamUse {
        ParamUse {
            param,
            part,
            gain: 1.0,
            via: None,
        }
    }

    fn controlled(index: usize, part: ParamPart, gain: f64) -> Control {
        Control::Param(param(index, part).scaled(gain))
    }

    #[test]
    fn control_finds_the_param_of_the_animation_at_the_time() {
        let mut properties = Properties::default();
        let x = properties.add(0.0);
        properties.change_base(x, Target::To(1.0), Some(param(0, ParamPart::X)));
        let mut moved = animation(1, Target::To(3.0));
        moved.param = Some(param(1, ParamPart::Number));
        properties.animate(x, moved);
        properties.resolve(&BLOCKS, 1.0);
        assert_eq!(properties.control(x, 1.0), controlled(0, ParamPart::X, 1.0));
        assert_eq!(
            properties.control(x, 3.0),
            controlled(1, ParamPart::Number, 1.0)
        );
        // Half way through a linear animation, the target moves it half as far.
        assert_eq!(
            properties.control(x, 2.5),
            controlled(1, ParamPart::Number, 0.5)
        );
    }

    #[test]
    fn relative_animations_in_code_follow_their_start_value() {
        let mut properties = Properties::default();
        let x = properties.add(0.0);
        properties.change_base(x, Target::To(1.0), Some(param(0, ParamPart::X)));
        properties.change_base(x, Target::By(2.0), None);
        properties.animate(x, animation(0, Target::By(1.0)));
        properties.animate(x, animation(1, Target::To(5.0)));
        properties.resolve(&BLOCKS, 1.0);
        assert_eq!(properties.base(x), 3.0);
        assert_eq!(properties.control(x, 1.0), controlled(0, ParamPart::X, 1.0));
        assert_eq!(properties.control(x, 2.5), Control::Code { block: Some(1) });
        properties.set_base(x, 0.0);
        assert_eq!(properties.control(x, 1.0), Control::Code { block: None });
    }

    #[test]
    fn scaling_in_code_scales_the_gain() {
        let mut properties = Properties::default();
        let scale = properties.add(1.0);
        let grow = Some(param(0, ParamPart::Number));
        properties.change_base(scale, Target::Times(2.0), grow);
        properties.change_base(scale, Target::Times(3.0), None);
        properties.animate(scale, animation(0, Target::Times(2.0)));
        properties.resolve(&BLOCKS, 1.0);
        // The base is 1 * grow * 3, and the animation doubles it by its end.
        let grow = |gain| controlled(0, ParamPart::Number, gain);
        assert_eq!(properties.control(scale, 0.0), grow(3.0));
        assert_eq!(properties.control(scale, 0.5), grow(4.5));
        assert_eq!(properties.control(scale, 1.5), grow(6.0));
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
