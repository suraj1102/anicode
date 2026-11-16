# Reference

Everything on one page.

## Shapes

| Function | Own options |
| --- | --- |
| `Rect {}` | `w`, `h`, `radius` |
| `Circle {}` | `r` |
| `Line {}` | `from`, `to` |
| `Path {}` | `points`, `closed` |
| `Math { "source" }` | `size` |
| `Tex { [[source]] }` | `size` |
| `Group {}` | none |

All of them also take `x`, `y`, `pos`, `scale`, `rotation`, `opacity`,
`fill`, `stroke` and `stroke_width`. See [Shapes](shapes.md) for defaults.

## Properties

| Shape | Properties |
| --- | --- |
| every shape | `x`, `y`, `scale`, `rotation`, `opacity`, `stroke_width` |
| `Rect` | `w`, `h`, `radius` |
| `Circle` | `r` |

Set a property by assignment (`box.x = 2`), compute it with
`box.x = derive(function() ... end)`, and animate it with
`animate(box.x, 2)`. `box.x:get()` returns its current number.

## Functions

| Function | Where | What it does |
| --- | --- | --- |
| `at "name" { ... }` | top level | a block of animations that starts at marker `name` |
| `play { ... }` | top level | an unnamed block that starts when the one before it ends |
| `animate(property, to)` | in a block | tweens a property or Value |
| `fade_in(shape)` | in a block | opacity 0 to 1; hidden before the block |
| `fade_out(shape)` | in a block | opacity to 0 |
| `move_to(shape, vec)` | in a block | moves to a point |
| `shift(shape, vec)`, `scale(shape, factor)`, `rotate(shape, angle)` | in a block | animate a move, a scale or a turn |
| `param("name", default)` | anywhere | a number or `vec` that the [params file](params.md) can set |
| `Value(start)` | anywhere | a number to animate and read |
| `derive(function)` | anywhere | a property computed every frame |
| `vec(x, y)` | anywhere | a 2D vector |
| `rgb("#rrggbb")` | anywhere | a color from hex |
| `require "name"` | anywhere | loads `name.lua` from the scene folder or its `lib` folder |

## Methods on animations

| Method | Meaning | Default |
| --- | --- | --- |
| `:over(seconds)` | length | 1 second |
| `:ease(name)` | `"linear"`, `"in"`, `"out"` or `"inout"` | `"inout"` |

## Methods on shapes

| Method | Outside a block | Inside a block |
| --- | --- | --- |
| `shape:move_to(vec)` | sets the position | animates to it |
| `shape:shift(vec)` | moves by `vec` | animates the move |
| `shape:scale(factor)` | multiplies the scale | animates it |
| `shape:rotate(angle)` | adds to the angle | animates it |
| `shape:add(child)` | puts `child` in the group; returns it | same |
| `shape:pos()` | returns the position as a `vec` | same |
| `shape:follow(leader, offset)` | keeps the shape at `offset` from `leader` every frame | same |

## Constants

| Name | Value |
| --- | --- |
| `ORIGIN`, `LEFT`, `RIGHT`, `UP`, `DOWN` | unit vectors |
| `WHITE`, `BLACK`, `GRAY`, `RED`, `ORANGE`, `YELLOW`, `GREEN`, `BLUE`, `PURPLE` | colors |

## Settings

All in `config/anicode.toml`.

| Key | Meaning |
| --- | --- |
| `render.width`, `render.height` | frame size for `render` and `new` |
| `render.fps` | frame rate for `render` and `new` |
| `render.background` | background as RGBA from 0 to 1; transparent by default |
| `render.frame_height` | scene units from the bottom to the top of the frame |
| `timing.default_block_seconds` | gap before a block that has no marker |
| `timing.default_animation_seconds` | length of an animation without `:over` |
| `golden.*` | settings for the golden-frame test |

In Shotcut, the frame size and rate come from the Shotcut project. The
editor's settings are in `config/editor.toml`; see
[Editor settings](params.md#editor-settings).

## Known limits

- `shape.scale` returns the `:scale()` method, not the property. You can
  still assign it (`shape.scale = 2`); animate it with `shape:scale()`.
- `fill` and `stroke` can only be set when a shape is created. Assigning
  `box.fill = RED` later does nothing.
- The points of a `Line` or `Path` cannot be animated.
- Methods like `shift` take no `:over` or `:ease`. Use `move_to` or
  `animate` for that.
- A number made from a `param` is not a plain Lua number. Compare it with
  `:get()`: `p:get() > 1`, not `p > 1`.

## Not available yet

`Text`, `Code`, `SVG`, `Image`, `Polygon`, `Arrow`, `write`, `morph`,
layout helpers like `next_to`, and the built-ins `Boids`, `FFT` and `Plot`.
