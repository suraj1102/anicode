# Math

`Math` typesets an equation written in
[Typst math syntax](https://typst.app/docs/reference/math/). `Tex` does the
same for LaTeX math. Both become ordinary shapes: you can move, scale, fade
and color them like any other shape.

## Math (Typst)

```lua
{{#include examples/math.lua}}
```

<video class="preview" src="media/math.mp4" autoplay loop muted playsinline></video>

## Tex (LaTeX)

```lua
{{#include examples/tex.lua}}
```

<video class="preview" src="media/tex.mp4" autoplay loop muted playsinline></video>

Write LaTeX in a `[[ ... ]]` string. In a normal `"..."` string Lua treats
each backslash as an escape, so `\frac` would need to be written `\\frac`.

`Tex` covers math commands and environments such as `pmatrix`, not packages
like TikZ.

## Options

The equation source comes first, without a name. Then any of the
[options every shape takes](shapes.md#options-every-shape-takes), plus:

| Option | Meaning | Default |
| --- | --- | --- |
| `size` | font size: the height of one em, in scene units | 0.5 |

With no `fill` or `stroke`, math is filled white. A mistake in the equation
is reported like any script error, for example
`scene.lua:3: math: unknown variable: nope`.

## Plain text

Quotes inside Typst math give upright text, which works for short labels
until a `Text` shape exists:

```lua
Math { '"gain"', size = 0.4 }
```
