# Anicode

Anicode turns Lua scene files into animations. You describe shapes and how
they move in code, and Anicode renders the frames.

```lua
{{#include examples/hello.lua}}
```

<video class="preview" src="media/hello.mp4" autoplay loop muted playsinline></video>

Every example in these docs is shown with its rendered output, like the one
above. The code is the whole scene: copy it into a `.lua` file and it renders
the same thing.

You can use the result in two ways:

- **On its own.** `anicode render` writes PNG frames, and ffmpeg turns them
  into a video. See [Getting started](getting-started.md).
- **Inside Shotcut.** A plugin plays a scene as a normal clip, and markers on
  the Shotcut timeline set when each part of the animation starts. See
  [Using Shotcut](shotcut.md).

## How a scene works

A scene has two parts:

1. **Create shapes.** Code at the top level of the file runs once and builds
   everything the scene shows.
2. **Animate them.** Each `at "name" { ... }` block marks a point in time.
   The animations inside the braces start there.

Any frame can be computed straight from its time, so you can render or scrub
to any point without playing the frames before it.
