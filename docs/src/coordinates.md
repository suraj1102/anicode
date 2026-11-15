# Coordinates and colors

## Coordinates

```lua
{{#include examples/coordinates.lua}}
```

<img class="preview" src="media/coordinates.png">

- The origin `(0, 0)` is the center of the frame.
- `x` points right and `y` points up.
- The frame is 8 units tall (`render.frame_height` in
  `config/anicode.toml`). At 16:9 it is about 14.2 units wide, so `x` runs
  from about -7.1 to 7.1 and `y` from -4 to 4.
- Angles are in radians. A positive angle turns counter-clockwise.

## Vectors

`vec(x, y)` makes a 2D vector. Vectors support `+`, `-`, unary `-`, and `*`
and `/` by a number:

```lua
local p = vec(1, 2) + vec(3, 0)   -- vec(4, 2)
local q = 2 * p                   -- vec(8, 4)
print(p)                          -- prints vec(4, 2)
```

Five vectors are predefined: `ORIGIN`, `LEFT`, `RIGHT`, `UP` and `DOWN`. They
are unit vectors, so `RIGHT * 3` is `vec(3, 0)`.

## Colors

```lua
{{#include examples/styles.lua}}
```

<img class="preview" src="media/styles.png">

- The named colors are `RED`, `ORANGE`, `YELLOW`, `GREEN`, `BLUE`, `PURPLE`,
  `GRAY`, `WHITE` and `BLACK` (top row, without `BLACK`).
- `rgb("#3b82f6")` reads a hex color.
- A color is a table `{r, g, b, a}` with each channel from 0 to 1. `a` is
  optional and defaults to 1.
- `opacity` fades the whole shape, fill and outline together (bottom right).
