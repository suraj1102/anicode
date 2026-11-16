# Params and the editor

## param

`param("name", default)` gives a number or a `vec` a name. The scene uses the
default until something sets the name in the scene's params file.

```lua
{{#include examples/params.lua}}
```

<video class="preview" src="media/params.mp4" autoplay loop muted playsinline></video>

## The params file

A scene `dot.lua` reads its params from `dot.params.lua` in the same folder.
It is plain Lua that returns a table of numbers and `{ x, y }` pairs:

```lua
-- Where the dot ends up.
return {
  ["dot.end"] = { 2, 1.5 },
  ["title.size"] = 0.8,
}
```

- A name that is not in the file uses the default from `param`.
- A name in the file must have the same kind of value as its default: a
  number for a number, a pair for a `vec`.
- `anicode render`, the Shotcut plugin and the editor all read it. The
  Shotcut plugin reloads when it changes, like it does for the scene file,
  so step one frame to see the change.
- The editor writes to this file. It changes only the lines of the params
  you drag, so comments and other lines stay as you wrote them.

## Doing math with params

A param works in `+`, `-`, `*` and `/` like a number, and keeps track of
itself through them. In `x = param("gap", 1) * 2 + 1`, dragging the shape
by 2 units changes `gap` by 1.

That tracking stops when the result can no longer be traced to one param:

- a param times a param, like `p * q`
- the sum of two params, like `p + q`
- any `math` function, like `math.sin(p)`

Those give a plain number. The shape still renders, but the editor cannot
drag it.

The tracked result is not a plain Lua number, so Lua will not compare it
with one: `p * 2 == 1` is always false, and `p * 2 > 1` is an error. Use
`:get()` for comparisons and loops:

```lua
local gap = param("gap", 1)
if (gap * 2):get() > 1 then ... end
for i = 1, param("count", 3):get() do ... end
```

## Placing shapes from other shapes

`shape:pos()` returns where a shape is when the scene is built, so other
shapes can be placed from it:

```lua
local dot = Circle { pos = param("dot.start", vec(1, 0)), fill = BLUE }
local label = Math { "a + b", pos = dot:pos() + param("label.offset", vec(0, -1)) }
```

Dragging the label changes `label.offset`, and the dot stays where it is.
Without a param of its own, like `pos = dot:pos() + vec(0, -1)`, the editor
does not guess: it says the label is placed from the dot, so you drag the dot
or add a param.

`dot:pos()` is read once. If the dot moves later in the scene, the label
stays behind. To keep a shape next to a moving one, use `follow`:

```lua
{{#include examples/follow_offset.lua}}
```

<video class="preview" src="media/follow_offset.mp4" autoplay loop muted playsinline></video>

`shape:follow(leader, offset)` sets the shape's `x` and `y` with `derive`,
so it stays at `offset` from `leader` on every frame. The offset defaults to
`vec(0, 0)`. Both shapes should be in the same group. A shape that follows
cannot also be animated: `move_to(label, ...)` makes the scene fail with
`animates a value that is set with derive`. Move the leader instead.

## The editor

The editor is a preview window for one scene:

```bash
cargo run --release -p anicode-editor -- examples/drag.lua
```

- **Play and scrub.** Play or Space plays from the playhead. Click or drag
  in the bar under the preview to scrub. Each tick on the bar is the start
  of an `at` or `play` block, with its name. If a saved Shotcut project uses
  the scene, its markers set the block times, the same as in Shotcut.
- **Reload.** Saving the scene, its params file or the Shotcut project
  reloads the preview. If the script has an error, the message shows in red
  under the bar and the last working frame stays.
- **Drag.** Drag a shape to move it. Hold Shift to scale it, or Alt to
  rotate it. The editor changes the param behind the value and writes the
  params file.
- **Inspector.** The panel on the right lists every param with a field you
  can drag or type into, and a "reset" button for params set in the file,
  which removes them so the default applies again. Under it, "Selected"
  shows the clicked shape's properties at the playhead and the param behind
  each one. Hover "not a param" to see why.
- **Undo.** Cmd+Z undoes a change and Cmd+Shift+Z redoes it (Ctrl on
  Linux). The History section lists every state. Undoing and then changing
  something starts a branch; the old branch stays and you can click any
  state to go back to it. Changes you make to the params file by hand show
  up as "edited outside the editor". The history is saved in
  `.anicode/history/`, so it is still there when you open the editor again.

### What a drag changes

A drag changes the param that sets the value at the playhead:

- Before any animation of the value has started, that is the param behind
  the shape's own value, like `pos = param("dot.start", ...)`.
- After an animation has started, it is the param behind that animation's
  target, like `move_to(dot, param("dot.end", ...))`. Part way through the
  animation, the target moves further than the cursor, so the shape stays
  under it. Right at the start of an animation the shape barely follows its
  target, so the editor asks you to scrub later.
- A relative change with a number in the code, like `shift(dot, RIGHT)`,
  passes the drag on to the value it started from.
- A shape with no param of its own drags the nearest group above it that has
  one, so a shape inside a component moves the component.
- A follower drags by the param in its offset.

When no param sets the value, the editor says what does instead, for example
`Circle on line 5: x is a number in the code; set it with param() to drag
it`.

### Editor settings

All in `config/editor.toml`.

| Key | Meaning |
| --- | --- |
| `preview.scale` | preview size as a fraction of `render.width` and `render.height` |
| `drag.step` | dragged values are rounded to this, in scene units or radians |
| `drag.hit_slop` | preview pixels around a thin line that still count as clicking it |
| `drag.min_gain` | how far an animation must be before its target can be dragged |
| `watch.poll_seconds` | how often to check the files for changes |
| `history.folder` | where the undo history is saved, under the project folder |
| `inspector.speed` | how much a param field changes per pixel of drag |
| `inspector.open_command` | command for "Open in editor", like `"code -g {file}:{line}"`; empty hides the button |
