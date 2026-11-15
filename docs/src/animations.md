# Animations

Animations only work inside an `at` block. Each one starts at its block and
lasts 1 second unless you say otherwise. Several animations in one block run
at the same time.

## fade_in and fade_out

```lua
{{#include examples/fade.lua}}
```

<video class="preview" src="media/fade.mp4" autoplay loop muted playsinline></video>

`fade_in` also hides the shape until its block starts. `fade_out` fades to 0
from whatever the opacity is.

## move_to and shift

```lua
{{#include examples/move.lua}}
```

<video class="preview" src="media/move.mp4" autoplay loop muted playsinline></video>

`move_to(shape, vec)` goes to a point. `shape:shift(vec)` moves by an offset
from where the shape is at that moment.

## scale and rotate

```lua
{{#include examples/scale_rotate.lua}}
```

<video class="preview" src="media/scale_rotate.mp4" autoplay loop muted playsinline></video>

`shape:scale(factor)` multiplies the size. `shape:rotate(angle)` adds to the
angle, in radians.

## animate

`animate(property, to)` tweens any number on a shape, or a
[Value](derive.md):

```lua
{{#include examples/animate.lua}}
```

<video class="preview" src="media/animate.mp4" autoplay loop muted playsinline></video>

The properties you can animate are `x`, `y`, `rotation`, `opacity` and
`stroke_width` on every shape, `w`, `h` and `radius` on `Rect`, and `r` on
`Circle`. Animate the scale with the `:scale()` method; `animate(shape.scale,
...)` does not work.

## :over sets the length

```lua
{{#include examples/over.lua}}
```

<video class="preview" src="media/over.mp4" autoplay loop muted playsinline></video>

## :ease sets the speed curve

```lua
{{#include examples/ease.lua}}
```

<video class="preview" src="media/ease.mp4" autoplay loop muted playsinline></video>

| Easing | Motion |
| --- | --- |
| `"linear"` | constant speed |
| `"in"` | starts slow, ends fast |
| `"out"` | starts fast, ends slow |
| `"inout"` | slow at both ends; the default |

## Functions and methods

`fade_in`, `fade_out`, `move_to` and `animate` are functions. They return an
animation, so `:over` and `:ease` can follow them.

`shift`, `scale`, `rotate` and `move_to` are also methods on a shape. Inside a
block they animate with the default length and easing. They return the
shape instead of an animation, so they chain but take no `:over` or `:ease`:

```lua
at "zoom" { gain:scale(1.4):shift(LEFT * 0.5) }   -- 1 second, "inout"
at "slow" { move_to(gain, ORIGIN):over(3) }       -- use the function for :over
```

Outside a block the methods change the shape right away, which is handy for
placing it:

```lua
local box = Rect { fill = BLUE }
box:shift(UP * 2):scale(0.5)
```

## Loops

Lua does not allow a `for` loop inside the braces of a block. Put the loop
in a function and call that function in the block, as `over.lua` and
`ease.lua` above do with `go()` and `race()`.
