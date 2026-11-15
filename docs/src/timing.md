# Timing

```lua
{{#include examples/timing.lua}}
```

<video class="preview" src="media/timing.mp4" autoplay loop muted playsinline></video>

How the times are worked out:

- Blocks run in the order they appear in the scene.
- A block starts at the [Shotcut marker](shotcut.md) with its name, if
  there is one. Otherwise it starts 2 seconds after the previous block
  (`timing.default_block_seconds`). The first block starts at 0.
- An animation starts when its block starts and lasts 1 second
  (`timing.default_animation_seconds`), or the time given with `:over`.
- An animation does not stop at the next block. Above, the blue dot is still
  moving when the orange one starts.
- Before its animation starts, a property keeps its earlier value. After the
  animation ends, it keeps the final value.
- The scene ends 2 seconds after the last block starts, or when the last
  animation ends, whichever is later.
- Blocks cannot be nested.

To make something start later, give it its own block. There is no delay
inside a block.

If the same property is animated twice in one block, the later animation
wins.
