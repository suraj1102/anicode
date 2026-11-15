//! The Lua scene API: shapes, `at`, `play`, `animate`, `derive` and friends.
//! Lua only holds handles (ids); Rust owns every object in the scene.

use std::cell::{Cell, RefCell, RefMut};
use std::rc::Rc;

use anicode_core::kurbo::{Affine, Point};
use anicode_core::{Animation, Block, Color, Easing, Frame, NodeId, PropertyId, Scene, Target};
use mlua::{
    AnyUserData, Error, Function, Lua, MetaMethod, Result, Table, UserData, UserDataMethods, Value,
};

/// Registry key of the Lua table that holds derive functions. Keeping them in
/// Lua, not in Rust, avoids a reference cycle between Rust and the Lua state.
const DERIVES: &str = "anicode_derives";

#[derive(Default)]
pub(crate) struct State {
    pub scene: RefCell<Scene>,
    pub frame: RefCell<Option<Frame>>,
    /// The `at` block whose { } is being read.
    pub current_block: Cell<Option<usize>>,
    /// Lua reads `play { fade_in(a) }` inside out: `fade_in` runs before
    /// `play`. So the first animation function outside an `at` block opens
    /// the next `play` block, which `play` then closes.
    pub pending_play: Cell<Option<PendingPlay>>,
    play_count: Cell<usize>,
}

#[derive(Clone, Copy)]
pub(crate) struct PendingPlay {
    block: usize,
    /// Where the first animation was written, for the error if no `play`
    /// follows.
    pub line: Option<usize>,
    pub action: &'static str,
}

impl State {
    fn scene_mut(&self) -> Result<RefMut<'_, Scene>> {
        self.scene
            .try_borrow_mut()
            .map_err(|_| Error::runtime("the scene cannot be changed while a frame renders"))
    }

    /// The block an animation function adds to: the open `at` block, or the
    /// coming `play` block.
    fn animation_block(&self, lua: &Lua, action: &'static str) -> Result<usize> {
        if let Some(block) = self.current_block.get() {
            return Ok(block);
        }
        if let Some(pending) = self.pending_play.get() {
            return Ok(pending.block);
        }
        let block = self.add_play_block()?;
        let line = lua.inspect_stack(1, |debug| debug.current_line()).flatten();
        self.pending_play.set(Some(PendingPlay {
            block,
            line,
            action,
        }));
        Ok(block)
    }

    /// Unnamed `play` blocks are numbered 1, 2, 3, so markers can name them.
    fn add_play_block(&self) -> Result<usize> {
        let number = self.play_count.get() + 1;
        self.play_count.set(number);
        let mut scene = self.scene_mut()?;
        scene.blocks.push(Block {
            name: number.to_string(),
            seconds: None,
        });
        Ok(scene.blocks.len() - 1)
    }
}

type Shared = Rc<State>;

pub(crate) fn register(lua: &Lua, state: &Shared) -> Result<()> {
    let globals = lua.globals();
    lua.set_named_registry_value(DERIVES, lua.create_table()?)?;

    for kind in ["Group", "Rect", "Circle", "Line", "Path", "Math", "Tex"] {
        let state = state.clone();
        let constructor = lua.create_function(move |lua, options: Option<Table>| {
            create_node(lua, &state, kind, options)
        })?;
        globals.set(kind, constructor)?;
    }

    let shared = state.clone();
    globals.set(
        "Value",
        lua.create_function(move |_, initial: f64| {
            let id = shared.scene_mut()?.properties.add(initial);
            Ok(PropertyHandle {
                state: shared.clone(),
                id,
            })
        })?,
    )?;

    let shared = state.clone();
    globals.set(
        "at",
        lua.create_function(move |lua, name: String| {
            if shared.current_block.get().is_some() {
                return Err(Error::runtime(
                    "at blocks cannot be nested (is a { } missing after an earlier at?)",
                ));
            }
            if let Some(pending) = shared.pending_play.get() {
                return Err(outside_block_error(pending));
            }
            let block = {
                let mut scene = shared.scene_mut()?;
                scene.blocks.push(Block {
                    name,
                    seconds: None,
                });
                scene.blocks.len() - 1
            };
            shared.current_block.set(Some(block));
            let state = shared.clone();
            lua.create_function(move |lua, options: Table| {
                state.current_block.set(None);
                apply_block_options(lua, &state, block, "at", &options)
            })
        })?,
    )?;

    let shared = state.clone();
    globals.set(
        "play",
        lua.create_function(move |lua, options: Table| {
            if shared.current_block.get().is_some() {
                return Err(Error::runtime("play cannot be inside an at block"));
            }
            let block = match shared.pending_play.take() {
                Some(pending) => pending.block,
                None => shared.add_play_block()?,
            };
            for value in options.sequence_values::<Value>() {
                if let Value::UserData(data) = value?
                    && data.is::<NodeHandle>()
                {
                    return Err(Error::runtime(
                        "a method like box:shift(UP) inside play changes the value right away; \
                         write shift(box, UP) to animate it",
                    ));
                }
            }
            apply_block_options(lua, &shared, block, "play", &options)
        })?,
    )?;

    let shared = state.clone();
    globals.set(
        "animate",
        lua.create_function(move |lua, (property, to): (AnyUserData, f64)| {
            let id = property.borrow::<PropertyHandle>()?.id;
            let block = shared.animation_block(lua, "animate")?;
            animation_handle(&shared, block, &[(id, Target::To(to))])
        })?,
    )?;

    let shared = state.clone();
    globals.set(
        "fade_in",
        lua.create_function(move |lua, node: AnyUserData| {
            let opacity = node.borrow::<NodeHandle>()?.property("opacity");
            let block = shared.animation_block(lua, "fade_in")?;
            shared.scene_mut()?.properties.set_base(opacity, 0.0);
            animation_handle(&shared, block, &[(opacity, Target::To(1.0))])
        })?,
    )?;

    let shared = state.clone();
    globals.set(
        "fade_out",
        lua.create_function(move |lua, node: AnyUserData| {
            let opacity = node.borrow::<NodeHandle>()?.property("opacity");
            let block = shared.animation_block(lua, "fade_out")?;
            animation_handle(&shared, block, &[(opacity, Target::To(0.0))])
        })?,
    )?;

    let shared = state.clone();
    globals.set(
        "move_to",
        lua.create_function(move |lua, (node, to): (AnyUserData, Table)| {
            let node = node.borrow::<NodeHandle>()?.clone();
            let (x, y) = vec_xy(lua, &to)?;
            let block = shared.animation_block(lua, "move_to")?;
            animation_handle(
                &shared,
                block,
                &[
                    (node.property("x"), Target::To(x)),
                    (node.property("y"), Target::To(y)),
                ],
            )
        })?,
    )?;

    // Function forms of the node methods, for use inside play { }.
    let shared = state.clone();
    globals.set(
        "shift",
        lua.create_function(move |lua, (node, by): (AnyUserData, Table)| {
            let node = node.borrow::<NodeHandle>()?.clone();
            let (x, y) = vec_xy(lua, &by)?;
            let block = shared.animation_block(lua, "shift")?;
            animation_handle(
                &shared,
                block,
                &[
                    (node.property("x"), Target::By(x)),
                    (node.property("y"), Target::By(y)),
                ],
            )
        })?,
    )?;

    let shared = state.clone();
    globals.set(
        "scale",
        lua.create_function(move |lua, (node, factor): (AnyUserData, f64)| {
            let scale = node.borrow::<NodeHandle>()?.property("scale");
            let block = shared.animation_block(lua, "scale")?;
            animation_handle(&shared, block, &[(scale, Target::Times(factor))])
        })?,
    )?;

    let shared = state.clone();
    globals.set(
        "rotate",
        lua.create_function(move |lua, (node, angle): (AnyUserData, f64)| {
            let rotation = node.borrow::<NodeHandle>()?.property("rotation");
            let block = shared.animation_block(lua, "rotate")?;
            animation_handle(&shared, block, &[(rotation, Target::By(angle))])
        })?,
    )?;

    globals.set(
        "derive",
        lua.create_function(|lua, function: Function| {
            let derives: Table = lua.named_registry_value(DERIVES)?;
            derives.raw_push(function)?;
            Ok(DeriveHandle(derives.raw_len() - 1))
        })?,
    )?;

    Ok(())
}

/// Reads a property at the current frame's time, running its derive function
/// if it has one. Outside a frame it returns the value set by the scene code.
pub(crate) fn property_value(lua: &Lua, state: &State, id: PropertyId) -> Result<f64> {
    let derive = {
        let scene = state.scene.borrow();
        let frame = state.frame.borrow();
        match (scene.properties.derive(id), frame.as_ref()) {
            (None, Some(frame)) => return Ok(scene.properties.value_at(id, frame.time)),
            (None, None) => return Ok(scene.properties.base(id)),
            (Some(_), None) => {
                return Err(Error::runtime(
                    "a derived value can only be read while a frame renders",
                ));
            }
            (Some(derive), Some(frame)) => {
                if let Some(value) = frame.cached(id) {
                    return Ok(value);
                }
                derive
            }
        }
    };

    if let Some(frame) = state.frame.borrow_mut().as_mut() {
        frame
            .start(id)
            .map_err(|_| Error::runtime("a derive depends on its own value"))?;
    }
    let derives: Table = lua.named_registry_value(DERIVES)?;
    let function: Function = derives.raw_get(derive + 1)?;
    let value = number(lua, &function.call::<Value>(())?)?;
    if let Some(frame) = state.frame.borrow_mut().as_mut() {
        frame.finish(id, value);
    }
    Ok(value)
}

fn create_node(
    lua: &Lua,
    state: &Shared,
    kind: &str,
    options: Option<Table>,
) -> Result<AnyUserData> {
    let options = match options {
        Some(options) => options,
        None => lua.create_table()?,
    };
    let shape_keys: &[&str] = match kind {
        "Rect" => &["w", "h", "radius"],
        "Circle" => &["r"],
        "Line" => &["from", "to"],
        "Path" => &["points", "closed"],
        "Math" | "Tex" => &["size"],
        _ => &[],
    };
    let common_keys = [
        "x",
        "y",
        "pos",
        "scale",
        "rotation",
        "opacity",
        "fill",
        "stroke",
        "stroke_width",
    ];
    for pair in options.pairs::<Value, Value>() {
        let (key, _) = pair?;
        // Math and Tex take their source as the first, unnamed value.
        let is_source = matches!(kind, "Math" | "Tex") && key == Value::Integer(1);
        let known = is_source
            || key
                .as_string()
                .and_then(|key| key.to_str().ok())
                .is_some_and(|key| shape_keys.contains(&&*key) || common_keys.contains(&&*key));
        if !known {
            return Err(Error::runtime(format!(
                "{kind} has no option {}",
                key.to_string()?
            )));
        }
    }

    let number_or = |key: &str, default: f64| -> Result<f64> {
        match options.get::<Value>(key)? {
            Value::Nil => Ok(default),
            value => number(lua, &value),
        }
    };

    let mut scene = state.scene_mut()?;
    let id = match kind {
        "Group" => scene.add_group(),
        "Rect" => scene.add_rect(
            number_or("w", 1.0)?,
            number_or("h", 1.0)?,
            number_or("radius", 0.0)?,
        ),
        "Circle" => scene.add_circle(number_or("r", 0.5)?),
        "Line" => scene.add_line(
            point(lua, &options.get::<Table>("from")?)?,
            point(lua, &options.get::<Table>("to")?)?,
        ),
        "Path" => {
            let points = options
                .get::<Table>("points")?
                .sequence_values::<Table>()
                .map(|vector| point(lua, &vector?))
                .collect::<Result<Vec<Point>>>()?;
            scene.add_path(
                points,
                options.get::<Option<bool>>("closed")?.unwrap_or(false),
            )
        }
        "Math" | "Tex" => {
            let source = options.get::<Option<String>>(1)?.ok_or_else(|| {
                Error::runtime(format!(
                    "{kind} needs its source first, like {kind} {{ \"x^2\" }}"
                ))
            })?;
            let outline = if kind == "Math" {
                anicode_text::typst_math(&source)
            } else {
                anicode_text::latex_math(&source)
            }
            .map_err(Error::runtime)?;
            // The outline is in ems; `size` is one em in scene units.
            let size = number_or("size", 0.5)?;
            scene.add_outline(Affine::scale(size) * outline)
        }
        _ => unreachable!("only known shapes are registered"),
    };

    let fill = options
        .get::<Option<Table>>("fill")?
        .map(color)
        .transpose()?;
    let mut stroke = options
        .get::<Option<Table>>("stroke")?
        .map(color)
        .transpose()?;
    let mut fill = fill;
    if fill.is_none() && stroke.is_none() {
        // Text reads best filled; other shapes default to outlines.
        if matches!(kind, "Math" | "Tex") {
            fill = Some([1.0; 4]);
        } else {
            stroke = Some([1.0; 4]);
        }
    }
    let node = &mut scene.nodes[id];
    node.fill = fill;
    node.stroke = stroke;

    let mut bases = vec![];
    if let Some(pos) = options.get::<Option<Table>>("pos")? {
        let (x, y) = vec_xy(lua, &pos)?;
        bases.extend([("x", x), ("y", y)]);
    }
    for key in ["x", "y", "scale", "rotation", "opacity", "stroke_width"] {
        if let Value::Nil = options.get::<Value>(key)? {
            continue;
        }
        bases.push((key, number_or(key, 0.0)?));
    }
    for (key, value) in bases {
        let property = scene.nodes[id]
            .property(key)
            .expect("every node has this property");
        scene.properties.set_base(property, value);
    }
    drop(scene);

    let handle = lua.create_userdata(NodeHandle {
        state: state.clone(),
        id,
    })?;
    // Holds fields that components store on a node, like `self.body`.
    handle.set_user_value(lua.create_table()?)?;
    Ok(handle)
}

/// Changes properties: as animations inside an `at` block, or immediately
/// outside one.
fn change(
    state: &State,
    block: Option<usize>,
    changes: &[(PropertyId, Target)],
) -> Result<Vec<(PropertyId, usize)>> {
    let mut scene = state.scene_mut()?;
    let mut added = vec![];
    for &(id, target) in changes {
        match block {
            Some(block) => {
                let animation = Animation {
                    block,
                    duration: None,
                    target,
                    easing: Easing::InOut,
                };
                added.push((id, scene.properties.animate(id, animation)));
            }
            None => {
                let base = scene.properties.base(id);
                scene.properties.set_base(id, target.apply(base));
            }
        }
    }
    Ok(added)
}

fn animation_handle(
    state: &Shared,
    block: usize,
    changes: &[(PropertyId, Target)],
) -> Result<AnimationHandle> {
    Ok(AnimationHandle {
        state: state.clone(),
        parts: change(state, Some(block), changes)?,
    })
}

/// Reads `seconds = n` and rejects other named options.
fn apply_block_options(
    lua: &Lua,
    state: &State,
    block: usize,
    keyword: &str,
    options: &Table,
) -> Result<()> {
    for pair in options.pairs::<Value, Value>() {
        let (key, value) = pair?;
        let Some(name) = key.as_string().and_then(|key| key.to_str().ok()) else {
            continue;
        };
        if &*name != "seconds" {
            return Err(Error::runtime(format!(
                "{keyword} has no option {}",
                &*name
            )));
        }
        let seconds = number(lua, &value)?;
        state.scene_mut()?.blocks[block].seconds = Some(seconds);
    }
    Ok(())
}

pub(crate) fn outside_block_error(pending: PendingPlay) -> Error {
    Error::runtime(match pending.line {
        Some(line) => format!(
            "{} on line {line} is not inside an at or play block",
            pending.action
        ),
        None => format!("{} is not inside an at or play block", pending.action),
    })
}

fn number(lua: &Lua, value: &Value) -> Result<f64> {
    match value {
        Value::Number(number) => Ok(*number),
        Value::Integer(number) => Ok(*number as f64),
        Value::UserData(data) if data.is::<PropertyHandle>() => {
            let property = data.borrow::<PropertyHandle>()?;
            property_value(lua, &property.state, property.id)
        }
        other => Err(Error::runtime(format!(
            "expected a number, got {}",
            other.type_name()
        ))),
    }
}

fn vec_xy(lua: &Lua, vector: &Table) -> Result<(f64, f64)> {
    Ok((
        number(lua, &vector.get::<Value>("x")?)?,
        number(lua, &vector.get::<Value>("y")?)?,
    ))
}

fn point(lua: &Lua, vector: &Table) -> Result<Point> {
    let (x, y) = vec_xy(lua, vector)?;
    Ok(Point::new(x, y))
}

fn color(table: Table) -> Result<Color> {
    Ok([
        table.get(1)?,
        table.get(2)?,
        table.get(3)?,
        table.get::<Option<f32>>(4)?.unwrap_or(1.0),
    ])
}

#[derive(Clone)]
struct NodeHandle {
    state: Shared,
    id: NodeId,
}

impl NodeHandle {
    fn find_property(&self, name: &str) -> Option<PropertyId> {
        self.state.scene.borrow().nodes[self.id].property(name)
    }

    fn property(&self, name: &str) -> PropertyId {
        self.find_property(name)
            .expect("every node has transform properties")
    }
}

impl UserData for NodeHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_function("add", |_, (this, child): (AnyUserData, AnyUserData)| {
            let parent = this.borrow::<NodeHandle>()?;
            let child_id = child.borrow::<NodeHandle>()?.id;
            parent
                .state
                .scene_mut()?
                .set_parent(child_id, parent.id)
                .map_err(Error::runtime)?;
            Ok(child)
        });

        methods.add_function("move_to", |lua, (this, to): (AnyUserData, Table)| {
            let node = this.borrow::<NodeHandle>()?.clone();
            let (x, y) = vec_xy(lua, &to)?;
            change(
                &node.state,
                node.state.current_block.get(),
                &[
                    (node.property("x"), Target::To(x)),
                    (node.property("y"), Target::To(y)),
                ],
            )?;
            Ok(this)
        });

        methods.add_function("shift", |lua, (this, by): (AnyUserData, Table)| {
            let node = this.borrow::<NodeHandle>()?.clone();
            let (x, y) = vec_xy(lua, &by)?;
            change(
                &node.state,
                node.state.current_block.get(),
                &[
                    (node.property("x"), Target::By(x)),
                    (node.property("y"), Target::By(y)),
                ],
            )?;
            Ok(this)
        });

        methods.add_function("scale", |_, (this, factor): (AnyUserData, f64)| {
            let node = this.borrow::<NodeHandle>()?.clone();
            change(
                &node.state,
                node.state.current_block.get(),
                &[(node.property("scale"), Target::Times(factor))],
            )?;
            Ok(this)
        });

        methods.add_function("rotate", |_, (this, angle): (AnyUserData, f64)| {
            let node = this.borrow::<NodeHandle>()?.clone();
            change(
                &node.state,
                node.state.current_block.get(),
                &[(node.property("rotation"), Target::By(angle))],
            )?;
            Ok(this)
        });

        methods.add_method("pos", |lua, this, ()| {
            let x = property_value(lua, &this.state, this.property("x"))?;
            let y = property_value(lua, &this.state, this.property("y"))?;
            lua.globals().get::<Function>("vec")?.call::<Value>((x, y))
        });

        methods.add_meta_function(
            MetaMethod::Index,
            |lua, (this, key): (AnyUserData, Value)| {
                let node = this.borrow::<NodeHandle>()?.clone();
                if let Some(id) = property_key(&node, &key) {
                    let handle = PropertyHandle {
                        state: node.state.clone(),
                        id,
                    };
                    return Ok(Value::UserData(lua.create_userdata(handle)?));
                }
                this.user_value::<Table>()?.raw_get::<Value>(key)
            },
        );

        methods.add_meta_function(
            MetaMethod::NewIndex,
            |_, (this, key, value): (AnyUserData, Value, Value)| {
                let node = this.borrow::<NodeHandle>()?.clone();
                match property_key(&node, &key) {
                    Some(id) => assign(&node.state, id, value),
                    None => this.user_value::<Table>()?.raw_set(key, value),
                }
            },
        );
    }
}

fn property_key(node: &NodeHandle, key: &Value) -> Option<PropertyId> {
    let name = key.as_string()?.to_str().ok()?;
    node.find_property(&name)
}

/// `node.x = 2` sets the value; `node.x = derive(fn)` computes it every frame.
fn assign(state: &State, id: PropertyId, value: Value) -> Result<()> {
    let mut scene = state.scene_mut()?;
    match value {
        Value::Number(number) => scene.properties.set_base(id, number),
        Value::Integer(number) => scene.properties.set_base(id, number as f64),
        Value::UserData(data) if data.is::<DeriveHandle>() => {
            let derive = data.borrow::<DeriveHandle>()?.0;
            scene.properties.set_derive(id, derive);
        }
        other => {
            return Err(Error::runtime(format!(
                "a property takes a number or derive(function), got {}",
                other.type_name()
            )));
        }
    }
    Ok(())
}

type Operator = fn(f64, f64) -> f64;

#[derive(Clone)]
struct PropertyHandle {
    state: Shared,
    id: PropertyId,
}

impl UserData for PropertyHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get", |lua, this, ()| {
            property_value(lua, &this.state, this.id)
        });

        let operators: [(MetaMethod, Operator); 4] = [
            (MetaMethod::Add, |a, b| a + b),
            (MetaMethod::Sub, |a, b| a - b),
            (MetaMethod::Mul, |a, b| a * b),
            (MetaMethod::Div, |a, b| a / b),
        ];
        for (method, operator) in operators {
            methods.add_meta_function(method, move |lua, (a, b): (Value, Value)| {
                Ok(operator(number(lua, &a)?, number(lua, &b)?))
            });
        }
        methods.add_meta_method(MetaMethod::Unm, |lua, this, ()| {
            Ok(-property_value(lua, &this.state, this.id)?)
        });
        methods.add_meta_method(MetaMethod::ToString, |lua, this, ()| {
            Ok(property_value(lua, &this.state, this.id)?.to_string())
        });
    }
}

struct AnimationHandle {
    state: Shared,
    parts: Vec<(PropertyId, usize)>,
}

impl AnimationHandle {
    fn update(&self, edit: impl Fn(&mut Animation)) -> Result<()> {
        let mut scene = self.state.scene_mut()?;
        for &(id, index) in &self.parts {
            edit(scene.properties.animation_mut(id, index));
        }
        Ok(())
    }
}

impl UserData for AnimationHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_function("over", |_, (this, seconds): (AnyUserData, f64)| {
            this.borrow::<AnimationHandle>()?
                .update(|animation| animation.duration = Some(seconds))?;
            Ok(this)
        });

        methods.add_function("ease", |_, (this, name): (AnyUserData, String)| {
            let easing = Easing::from_name(&name).ok_or_else(|| {
                Error::runtime(format!(
                    "unknown easing \"{name}\" (use linear, in, out or inout)"
                ))
            })?;
            this.borrow::<AnimationHandle>()?
                .update(|animation| animation.easing = easing)?;
            Ok(this)
        });
    }
}

struct DeriveHandle(usize);

impl UserData for DeriveHandle {}
