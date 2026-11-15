# Anicode

Anicode turns Lua scene files into animation frames. You can use it in two
ways:

1. **On its own**: the `anicode render` command writes one PNG file per
   frame. You turn the PNGs into a video with ffmpeg.
2. **Inside Shotcut**: a plugin lets Shotcut play a scene as a normal clip.
   You time the animation by placing markers on the Shotcut timeline.

Start with the first way. It needs nothing but Rust.

More docs:

- [docs/scene-api.md](docs/scene-api.md): how to write scenes (every
  function and option).
- [anicode-build-plan.md](anicode-build-plan.md): design and roadmap.

Text and math are being added and are not documented yet.

## Quick start

Run these from the repo root.

**1. Render an example to PNG files.**

```bash
cargo run --release -p anicode-cli -- render examples/orbit.lua --out frames
```

The first run compiles the project, which takes a few minutes. After that
the render itself takes about a second. When it finishes it prints one
line:

```
181 frames at 1920x1080: scene 0.01 ms, render 0.50 ms per frame (CPU)
```

Nothing opens and nothing plays. The result is the files
`frames/frame_00000.png` to `frames/frame_00180.png`.

> Always pass `--release`. Without it, cargo builds a debug version that is
> about 100 times slower. The same render takes over two minutes and prints
> nothing until the end, so it looks stuck.

**2. Turn the frames into a video and watch it.**

```bash
ffmpeg -y -framerate 29.97 -i frames/frame_%05d.png -pix_fmt yuv420p orbit.mp4
```

```bash
open orbit.mp4
```

Use the same frame rate as `render.fps` in `config/anicode.toml`. The scene
background is transparent, and in an MP4 it shows as black.

**3. Clear the folder before rendering another scene.**

`render` overwrites frames with the same number but never deletes old ones.
If the last scene was longer, its extra frames stay and end up in your
video.

```bash
rm -rf frames
```

## Write your own scene

Make a `scenes` folder in the repo root (`mkdir scenes`) and put a file
`scenes/hello.lua` in it:

```lua
local box = Rect { w = 3, h = 2, radius = 0.2, fill = BLUE }

at "intro" { fade_in(box) }
at "slide" { move_to(box, vec(3, 1)):over(1):ease("out") }
at "leave" { fade_out(box) }
```

Render and watch it:

```bash
rm -rf frames && cargo run -q --release -p anicode-cli -- render scenes/hello.lua --out frames
```

```bash
ffmpeg -y -framerate 29.97 -i frames/frame_%05d.png -pix_fmt yuv420p hello.mp4 && open hello.mp4
```

What the scene does:

- `Rect { ... }` creates a rounded box. It exists for the whole scene.
- Each `at "name" { ... }` marks a point in time. With no Shotcut markers,
  the blocks start 2 seconds apart (0 s, 2 s, 4 s), so this scene is 6
  seconds long.
- `fade_in`, `move_to` and `fade_out` are animations. Each one starts at its
  block and runs for 1 second, or for the time given with `:over`.

If the script has a mistake, `render` prints `error: file:line: message` and
writes no frames.

[docs/scene-api.md](docs/scene-api.md) explains every shape, animation and
option.

## Render options

```
anicode render SCENE --out DIR [--frames A..B] [--gpu] [--config PATH]
```

| Option | Meaning |
| --- | --- |
| `SCENE` | the `.lua` file to render |
| `--out DIR` | folder for the PNG files; created if missing |
| `--frames A..B` | render only frames A to B-1, for example `--frames 0..30` for the first second |
| `--gpu` | render on the GPU (Metal or Vulkan) instead of the CPU |
| `--config PATH` | settings file; default `config/anicode.toml`, relative to the folder you run from |

To skip `cargo run`, build once and call the binary directly:

```bash
cargo build --release -p anicode-cli
```

```bash
./target/release/anicode render examples/orbit.lua --out frames
```

`render` ignores Shotcut markers, so the blocks always start 2 seconds apart.

## What works today

- Shapes: `Rect`, `Circle`, `Line`, `Path`, `Group`.
- Animations: `at` blocks, `animate`, `fade_in`, `fade_out`, `move_to`,
  `:over`, `:ease`, and the `shift`, `scale` and `rotate` methods.
- `Value` and `derive` for numbers computed every frame.
- Reusable components loaded with `require`.
- `anicode render` (PNG frames) and `anicode new` (Shotcut wrapper).
- The Shotcut plugin: plays scenes, reloads on save, reads marker times,
  shows a red bar on script errors.

## Examples

| Scene | Shows |
| --- | --- |
| [examples/shape.lua](examples/shape.lua) | the smallest scene: a box, a child LED and a `Value` |
| [examples/basics.lua](examples/basics.lua) | shapes, groups, a component (`lib/knob.lua`), `derive` |
| [examples/easing.lua](examples/easing.lua) | the four easing curves, `:over`, method chains, `fade_out`, loops in blocks |
| [examples/orbit.lua](examples/orbit.lua) | `derive` with `math` functions, nested groups |
| [examples/mixer.lua](examples/mixer.lua) | a component (`lib/meter.lua`) whose segments light up through `derive` |

Each example also has a `.mlt` file for Shotcut. `examples/edit.mlt` is a
small Shotcut project with markers, for trying the marker workflow.

## Use a scene in Shotcut

This part needs the plugin, which is built with CMake.

### 1. Build the plugin

Install the tools.

macOS:

```bash
brew install mlt ninja cmake
```

Debian:

```bash
sudo apt install build-essential cmake ninja-build pkg-config libmlt-dev melt
```

Build:

```bash
cmake -S . -B build -G Ninja
```

```bash
cmake --build build
```

This makes `build/libmltanicode.so` (the plugin) and `build/anicode` (the
same CLI as above).

### 2. Install the plugin

**macOS.** The normal Shotcut.app only loads plugins from inside itself. Make
a copy of the app, link the plugin into the copy, and re-sign the copy. Run
these from the repo root, and redo them after each Shotcut update:

```bash
ditto /Applications/Shotcut.app ~/Applications/Shotcut-Anicode.app
```

```bash
xattr -dr com.apple.quarantine ~/Applications/Shotcut-Anicode.app
```

```bash
ln -sf "$PWD/build/libmltanicode.so" ~/Applications/Shotcut-Anicode.app/Contents/PlugIns/mlt/
```

```bash
codesign --force --deep --sign - --preserve-metadata=entitlements,flags ~/Applications/Shotcut-Anicode.app
```

From now on, open `~/Applications/Shotcut-Anicode.app`, not the normal
Shotcut. The plugin is a link to the build folder, so after a rebuild you
only need to restart Shotcut.

**Debian.** Link the plugin into the system MLT module folder:

```bash
sudo ln -sf "$PWD/build/libmltanicode.so" "$(pkg-config --variable=moduledir mlt-framework-7)/"
```

### 3. Make a clip file for the scene

Shotcut cannot open a `.lua` file. `anicode new` writes a small `.mlt` file
next to the scene that Shotcut can open:

```bash
./target/release/anicode new scenes/hello.lua
```

This writes `scenes/hello.mlt`. Drag it onto a video track in Shotcut.

- It needs `config/anicode.toml` in the scene's folder or a folder above it.
- It will not overwrite a `.mlt` file that already exists.

### 4. Time the scene with markers

In Shotcut, add a timeline marker for each `at` block and give it the block's
name. Then save the project.

For `hello.lua` with `at "slide" { ... }`, name the marker `slide` or
`hello:slide`. Use the `hello:` form when two scenes in one project have a
block with the same name.

- A block starts at its marker.
- A block with no marker starts 2 seconds after the one before it, so a
  scene plays even before you add any markers.
- Animations stay at the markers. If you move the clip on the timeline but
  not the markers, each block still starts where its marker is.
- The engine looks for the Shotcut project in the folder that holds
  `config/anicode.toml` and the folders below it. It uses the most recently
  saved project that contains the scene.

### 5. Edit and see the change

Save the `.lua` file or the Shotcut project, then step one frame or scrub in
Shotcut. The clip reloads. Shotcut does not redraw while paused, so the
change does not show until you move the playhead.

If the script has a mistake, the clip keeps showing the last working
version with a red bar across the top. The error message, like
`hello.lua:3: attempt to index nil`, is in Shotcut's application log. To
see it faster, render the scene with `anicode render`, which prints the
error.

The clip starts at the scene length. You can drag its end out to make it
longer, and the last frame holds.

## Settings

All settings are in `config/anicode.toml`.

| Key | Meaning |
| --- | --- |
| `render.width`, `render.height` | frame size for `render` and `new` |
| `render.fps` | frame rate for `render` and `new` |
| `render.background` | background color as RGBA from 0 to 1; transparent by default so clips layer over video in Shotcut |
| `render.frame_height` | how many scene units fit from the bottom to the top of the frame |
| `timing.default_block_seconds` | gap before an `at` block that has no marker |
| `timing.default_animation_seconds` | length of an animation without `:over` |
| `golden.*` | settings for the golden-frame test |

In Shotcut, the frame size and rate come from the Shotcut project instead.

## Tests

Run the unit tests and the golden-frame test:

```bash
cargo test --workspace
```

The golden-frame test renders the scenes listed under `[golden]` in the
config and compares them to the PNGs in `tests/golden/`. After an
intended change to those scenes, save new reference frames:

```bash
ANICODE_UPDATE_GOLDEN=1 cargo test -p anicode-cli --test golden
```

After `cmake --build build`, check that every example `.mlt` plays in
`melt`:

```bash
scripts/melt-smoke.sh
```
