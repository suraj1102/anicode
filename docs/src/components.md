# Components

A component is a Lua function that builds shapes inside a `Group`, links
them with `derive`, and returns the group. Anicode has no special support
for components: they are plain Lua, so you can write new ones without
rebuilding anything.

```lua
{{#include examples/component.lua}}
```

<video class="preview" src="media/component.mp4" autoplay loop muted playsinline></video>

- Store the parts on the group, like `self.progress`. Any field name that is
  not a property name holds a plain Lua value.
- Give the component a `Value` for each thing a scene should animate, like
  `progress`.

## Components in their own file

Put a component in a file and load it with `require`. Anicode looks for
`name.lua` next to the scene, then in the `lib` folder next to the scene.

```lua
{{#include ../../examples/lib/knob.lua}}
```

```lua
local Knob = require "knob"
local gain = Knob.new { pos = vec(-1.5, 0), color = RED }
at "turn" { animate(gain.value, 1) }
```

The [gallery](gallery.md) has two scenes that use components from
`examples/lib`: `basics.lua` and `mixer.lua`.

## Methods for every shape

The global table `Node` holds methods, written in Lua, that every shape
has. `follow` is one of them. A component library can add its own:

```lua
function Node.fade_to(self, opacity)
  self.opacity = opacity
  return self
end

box:fade_to(0.5)
```

A field stored on a shape, like `self.body`, is found before a `Node`
method of the same name.
