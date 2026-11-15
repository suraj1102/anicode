# Groups

A `Group` draws nothing itself. It holds other shapes, and they move, turn,
scale and fade with it.

```lua
{{#include examples/group.lua}}
```

<video class="preview" src="media/group.mp4" autoplay loop muted playsinline></video>

- `group:add(child)` puts `child` in the group and returns it, so you can
  create and add a shape in one line.
- A child's position is relative to its group. Above, the eyes sit at
  `(-0.45, 0.5)` from the robot's center wherever the robot goes.
- A group's `opacity` multiplies with its children's.
- Groups can hold groups. Adding a group to itself or to one of its own
  children is an error.
