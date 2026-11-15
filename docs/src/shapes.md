# Shapes

Each shape is made by a function that takes one table of options, such as
`Rect { w = 2, fill = BLUE }`. Every option is optional unless noted. A
misspelled option is an error (`Rect has no option widht`), so typos never
pass silently.

## Rect

```lua
{{#include examples/rect.lua}}
```

<img class="preview" src="media/rect.png">

| Option | Meaning | Default |
| --- | --- | --- |
| `w`, `h` | width and height | 1, 1 |
| `radius` | corner radius | 0 |

## Circle

```lua
{{#include examples/circle.lua}}
```

<img class="preview" src="media/circle.png">

| Option | Meaning | Default |
| --- | --- | --- |
| `r` | radius | 0.5 |

## Line and Path

```lua
{{#include examples/line_path.lua}}
```

<img class="preview" src="media/line_path.png">

| Shape | Option | Meaning |
| --- | --- | --- |
| `Line` | `from`, `to` | the two end points, as `vec`; required |
| `Path` | `points` | a list of `vec`, joined by straight lines; required |
| `Path` | `closed` | join the last point back to the first; default `false` |

The points of a `Line` or `Path` cannot be animated.

## Options every shape takes

| Option | Meaning | Default |
| --- | --- | --- |
| `x`, `y` | position of the shape's center | 0, 0 |
| `pos` | position as a `vec`; same as `x` and `y` | |
| `scale` | size factor | 1 |
| `rotation` | angle in radians, counter-clockwise | 0 |
| `opacity` | 0 (invisible) to 1 | 1 |
| `fill` | fill color | none |
| `stroke` | outline color | none |
| `stroke_width` | outline width in scene units | 0.04 |

A shape with neither `fill` nor `stroke` gets a white outline, so it is never
invisible by accident. `Math` and `Tex` get a white fill instead.

## Draw order

Shapes are drawn in the order they are created, so later shapes cover
earlier ones. Adding a shape to a [group](groups.md) does not change its
draw order.
