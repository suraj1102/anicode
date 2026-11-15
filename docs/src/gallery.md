# Gallery

Longer scenes from the `examples/` folder. Each one also has a `.mlt` file
for Shotcut.

## Knobs

Shapes, groups, a component from `examples/lib/knob.lua`, and `derive`.

```lua
{{#include ../../examples/basics.lua}}
```

<video class="preview" src="media/gallery-basics.mp4" autoplay loop muted playsinline></video>

## Easing race

The four easings, `:over`, method chains and `fade_out`.

```lua
{{#include ../../examples/easing.lua}}
```

<video class="preview" src="media/gallery-easing.mp4" autoplay loop muted playsinline></video>

## Orbit

`derive` with Lua's `math` functions, and a group inside a moving group.

```lua
{{#include ../../examples/orbit.lua}}
```

<video class="preview" src="media/gallery-orbit.mp4" autoplay loop muted playsinline></video>

## Mixer

A level meter component from `examples/lib/meter.lua`, with segments lit by
`derive`.

```lua
{{#include ../../examples/lib/meter.lua}}
```

```lua
{{#include ../../examples/mixer.lua}}
```

<video class="preview" src="media/gallery-mixer.mp4" autoplay loop muted playsinline></video>

## Equations

Typst and LaTeX math side by side.

```lua
{{#include ../../examples/math.lua}}
```

<video class="preview" src="media/gallery-math.mp4" autoplay loop muted playsinline></video>
