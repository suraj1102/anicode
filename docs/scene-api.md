# Scene API

A scene is one `.lua` file with two parts:

1. **Create shapes.** Code at the top level runs once and builds everything
   the scene shows.
2. **Animate them.** Each `at "name" { ... }` block is a stretch of time.
   The animations inside it run during that stretch.

To render a scene and watch it, see the Quick start in the
[README](../README.md).

This page lists what works today. Text and math are being added and are not
covered here yet.

## Quick reference

```lua
-- Shapes (all options are optional except Line's from and to)
local box  = Rect   { w = 3, h = 2, radius = 0.2, fill = BLUE, pos = vec(-2, 0) }
local dot  = Circle { r = 0.5, fill = RED, x = 2 }
local line = Line   { from = vec(-3, -2), to = vec(3, -2), stroke = GRAY }
local tri  = Path   { points = { vec(0, 1), vec(1, -1), vec(-1, -1) }, closed = true, fill = GREEN }
local grp  = Group  { pos = vec(0, 1) }
grp:add(dot)                          -- dot now moves with grp

-- Set values right away (outside any at block)
box.opacity = 0.5
box:shift(UP * 0.5)

-- A number you can animate, and a property computed from it every frame
local level = Value(0)
dot.opacity = derive(function() return level:get() end)

-- Animations (inside at blocks only)
at "intro" { fade_in(box), fade_in(line) }
at "move"  { move_to(box, vec(2, 1)):over(1):ease("out") }
at "grow"  { animate(level, 1), box:scale(1.5) }
at "end"   { fade_out(box) }
```

| I want to | Write |
| --- | --- |
| fade something in or out | `fade_in(node)`, `fade_out(node)` |
| move to a point | `move_to(node, vec(x, y))` |
| move by an offset | `node:shift(vec(dx, dy))` |
| grow or shrink | `node:scale(factor)` |
| turn | `node:rotate(radians)` |
| change any number | `animate(node.w, 5)` or `animate(value, 1)` |
| set how long it takes | add `:over(seconds)` (default 1 second) |
| change the speed curve | add `:ease("linear")`, `"in"`, `"out"` or `"inout"` |
| link one value to another | `node.x = derive(function() return ... end)` |
| start something later | put it in a later `at` block |

## Coordinates and units

- The origin is the center of the frame. `x` points right, `y` points up.
- Lengths are in scene units. The frame is `frame_height` units tall (8 by
  default, set in `config/anicode.toml`). At 16:9 that makes it about 14.2
  units wide, so `x` runs from about -7.1 to 7.1 and `y` from -4 to 4.
- Angles are in radians. A positive angle turns counter-clockwise.
- Colors are `{r, g, b, a}` tables with channels from 0 to 1. `a` is optional.

## Shapes

Every constructor takes one table of options. An unknown option is an error
(`Rect has no option widht`), which catches typos.

| Constructor | Shape options | Defaults |
| --- | --- | --- |
| `Group {}` | none; holds other nodes | |
| `Rect {}` | `w`, `h`, `radius` (corner radius) | 1, 1, 0 |
| `Circle {}` | `r` | 0.5 |
| `Line {}` | `from`, `to` (both `vec`) | required |
| `Path {}` | `points` (list of `vec`), `closed` (bool) | `closed = false` |

Options every shape takes:

| Option | Meaning | Default |
| --- | --- | --- |
| `x`, `y` | position | 0, 0 |
| `pos` | position as a `vec`, same as `x` and `y` | |
| `scale` | size factor | 1 |
| `rotation` | angle in radians | 0 |
| `opacity` | 0 to 1; multiplies down to children | 1 |
| `fill` | fill color | none |
| `stroke` | outline color | none |
| `stroke_width` | outline width in scene units | 0.04 |

A shape with neither `fill` nor `stroke` gets a white outline, so it is never
invisible by accident.

`Path` joins its points with straight lines. For a curve, use many points:

```lua
local points = {}
for i = 0, 60 do
  local x = -3 + 6 * i / 60
  points[#points + 1] = vec(x, math.sin(2 * x))
end
local wave = Path { points = points, stroke = GREEN, stroke_width = 0.05 }
```

### Draw order

Shapes are drawn in the order they are created, later ones on top. Adding a
shape to a group does not change its draw order.

## Groups

`group:add(child)` makes `child` move, rotate, scale and fade with `group`.
The child's position becomes relative to the group. `add` returns the child,
so you can create and add in one line:

```lua
local panel = Group { pos = vec(-2, 0) }
local body = panel:add(Rect { w = 3, h = 2, fill = GRAY })
local led = panel:add(Circle { r = 0.15, y = 0.6, fill = RED })
```

Adding a group to itself or to one of its own children is an error.

## Properties

These are the animatable numbers on a node:

| Node | Properties |
| --- | --- |
| every node | `x`, `y`, `scale`, `rotation`, `opacity`, `stroke_width` |
| `Rect` | `w`, `h`, `radius` |
| `Circle` | `r` |

Write them by assignment and read them as fields:

```lua
box.x = 2
box.opacity = 0.5
animate(box.w, 5)   -- box.w is a handle to the property
```

Known limits:

- `node.scale` reads the `:scale()` method, not the property. You can still
  assign it (`node.scale = 2`) and set it as an option, but animate it with
  the `:scale()` method, not `animate(node.scale, ...)`.
- `fill` and `stroke` can only be set when the shape is created. Assigning
  `box.fill = RED` later stores a plain field and has no effect on drawing.
- The points of a `Line` or `Path` cannot be animated.

### Your own fields

Any other key stores a plain Lua value on the node. Components use this to
keep their parts (`self.body`, `self.level`):

```lua
local knob = Group {}
knob.label = "gain"
knob.body = knob:add(Circle { r = 0.6, fill = RED })
```

## Values

`Value(initial)` makes a number that is not attached to a shape. Animate it
and read it from `derive` functions. This is how a component exposes one knob
that drives many shapes.

```lua
local level = Value(0)
at "rise" { animate(level, 1) }
```

A value (or any property handle) works in `+`, `-`, `*`, `/` and unary `-`,
and the result is a plain number. For anything else, such as comparisons or
`math.sin`, read the number with `:get()`:

```lua
level * 0.5              -- fine
math.sin(level:get())    -- math functions need :get()
level:get() > 0.5        -- comparisons need :get()
```

## derive

`derive(function)` computes a property every frame. It replaces Manim's
updaters. The function can read other properties, values and derived
properties:

```lua
led.opacity = derive(function() return 0.2 + 0.8 * level end)

planet.x = derive(function() return 3 * math.cos(angle:get()) end)
planet.y = derive(function() return 3 * math.sin(angle:get()) end)
```

Rules:

- A derived property cannot also be animated. The scene fails to load with
  `at "intro" animates a value that is set with derive`.
- A derive that depends on itself, directly or through others, fails with
  `a derive depends on its own value`.
- A derived value can only be read while a frame renders, so reading one at
  the top level of the scene is an error.

## at blocks and timing

```lua
at "intro" { fade_in(box), fade_in(label) }
```

`at "name"` opens a block and the `{ }` after it closes it. Animations
created inside the braces belong to that block.

- Blocks run in the order the scene declares them.
- A block starts at the Shotcut marker with the same name, if one is set (see
  the README). Otherwise it starts `default_block_seconds` (2 by default)
  after the previous block. The first block starts at 0.
- A block only sets when its animations start. Each animation runs for 1
  second (`default_animation_seconds`) unless you give it `:over(seconds)`.
  It does not stretch to the next block, and it may run past the next
  block's start.
- The scene ends `default_block_seconds` after the last block starts, or
  when the last animation finishes, whichever is later.
- Before its block starts, an animated property keeps its earlier value.
  After the animation ends, it keeps the final value.
- Blocks cannot be nested.

Loops cannot go inside the braces. Put them in a function that the block
calls:

```lua
local function show_all()
  for _, dot in ipairs(dots) do fade_in(dot) end
end

at "show" { show_all() }
```

## Animations

These work only inside an `at` block:

| Function | What it does |
| --- | --- |
| `animate(property, to)` | tweens a property or `Value` to `to` |
| `fade_in(node)` | opacity from 0 to 1; the node is hidden before the block |
| `fade_out(node)` | opacity to 0 |
| `move_to(node, vec)` | moves the node's position to `vec` |

Each returns an animation that takes two chained settings:

| Method | Meaning | Default |
| --- | --- | --- |
| `:over(seconds)` | how long the animation runs, from the block start | 1 second (`timing.default_animation_seconds`) |
| `:ease(name)` | `"linear"`, `"in"`, `"out"` or `"inout"` (cubic curves) | `"inout"` |

```lua
at "turn" { animate(knob.value, 1):over(1.5):ease("out") }
```

An animation always starts at the start of its block. To start something
later, give it its own block.

If one property is animated twice in the same block, the later one wins.

## Node methods

| Method | Outside a block | Inside a block |
| --- | --- | --- |
| `node:move_to(vec)` | sets the position | animates to it |
| `node:shift(vec)` | moves by `vec` | animates the move |
| `node:scale(factor)` | multiplies the scale | animates it |
| `node:rotate(angle)` | adds to the rotation | animates it |
| `node:add(child)` | parents `child`; returns it | same |
| `node:pos()` | returns the position as a `vec` | same |

The first four return the node, so they chain:

```lua
gain:scale(1.4):shift(LEFT * 0.5)
```

Inside a block they always run for the default 1 second with `"inout"` easing,
because they return the node, not an animation. When you need `:over` or
`:ease`, use the global `move_to` or `animate` instead.

## Vectors, directions and colors

These come from `lua/stdlib/prelude.lua`, which loads before every scene.

- `vec(x, y)` makes a vector. Vectors support `+`, `-`, unary `-`, `*` and
  `/` by a number, and `tostring`.
- Directions: `ORIGIN`, `LEFT`, `RIGHT`, `UP`, `DOWN` (unit vectors).
- `rgb("#ff8800")` makes a color from a hex string.
- Named colors: `WHITE`, `BLACK`, `GRAY`, `RED`, `ORANGE`, `YELLOW`,
  `GREEN`, `BLUE`, `PURPLE`.

A color with transparency is a plain table: `{1, 0, 0, 0.5}`.

## Components and require

A component is a Lua function that builds shapes inside a `Group` and links
them with `derive`. It lives only in Lua, so you can write new ones without
rebuilding anything.

`require "name"` looks for `name.lua` next to the scene, then in the `lib`
folder next to the scene.

```lua
-- lib/knob.lua
local Knob = {}

function Knob.new(opts)
  local self = Group { pos = opts.pos }
  self.value = Value(opts.value or 0)
  self.body = self:add(Circle { r = 0.6, fill = opts.color })
  self.dial = self:add(Group {})
  self.dial:add(Rect { w = 0.08, h = 0.4, y = 0.3, fill = WHITE })
  self.dial.rotation = derive(function() return 2.4 - 4.8 * self.value end)
  return self
end

return Knob
```

```lua
-- scene.lua
local Knob = require "knob"
local gain = Knob.new { pos = vec(-1.5, 0), color = RED }
at "turn" { animate(gain.value, 1) }
```

## Errors

Script errors read `file:line: message`, for example
`scene.lua:12: fade_in must be called inside an at block`. The CLI prints
them. In Shotcut the clip keeps showing the last scene that loaded, with a
red bar across the top, and the message goes to the Shotcut log.

## Not available yet

These are in the build plan but not in the engine yet: `Text`, `Math`,
`Code`, `SVG`, `Image`, `Polygon`, `Arrow`, `param`, `write`, `morph`,
layout helpers like `next_to` and `below`, and the built-ins `Boids`, `FFT`
and `Plot`.
