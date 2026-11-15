# Getting started

You need Rust (through rustup) and ffmpeg. Run every command from the root of
the repository.

## Render a scene

```bash
cargo run --release -p anicode-cli -- render examples/orbit.lua --out frames
```

The first run compiles the project, which takes a few minutes. After that a
render takes about a second. When it finishes it prints one line:

```text
181 frames at 1920x1080: scene 0.01 ms, render 0.50 ms per frame (CPU)
```

Nothing opens and nothing plays. The result is the files
`frames/frame_00000.png`, `frames/frame_00001.png` and so on.

> **Always pass `--release`.** Without it, cargo builds a debug version that
> is about 100 times slower. The render then takes minutes and prints nothing
> until the end, so it looks stuck.

## Watch it

Turn the frames into a video and open it:

```bash
ffmpeg -y -framerate 29.97 -i frames/frame_%05d.png -pix_fmt yuv420p orbit.mp4
```

```bash
open orbit.mp4
```

Use the frame rate from `render.fps` in `config/anicode.toml`. Scenes have a
transparent background, which shows as black in an MP4.

`render` never deletes old frames. Before you render a different scene,
clear the folder, or the extra frames from a longer scene end up in your
video:

```bash
rm -rf frames
```

## Write your own scene

Make a `scenes` folder and save this as `scenes/hello.lua`:

```lua
{{#include examples/hello.lua}}
```

<video class="preview" src="media/hello.mp4" autoplay loop muted playsinline></video>

Render it and watch it:

```bash
rm -rf frames && cargo run -q --release -p anicode-cli -- render scenes/hello.lua --out frames
```

```bash
ffmpeg -y -framerate 29.97 -i frames/frame_%05d.png -pix_fmt yuv420p hello.mp4 && open hello.mp4
```

What each line does:

- `Rect { ... }` creates a blue box. Shapes exist for the whole scene.
- `at "intro"`, `at "slide"` and `at "leave"` are three points in time. With
  no Shotcut markers, they are 2 seconds apart: 0 s, 2 s and 4 s.
- `fade_in`, `move_to` and `fade_out` are animations. Each starts at its
  block and lasts 1 second, unless `:over(seconds)` says otherwise.

If the scene has a mistake, `render` prints `error: file:line: message` and
writes no frames:

```text
error: scenes/hello.lua:4: fade_in must be called inside an at block
```

## Render options

```text
anicode render SCENE --out DIR [--frames A..B] [--gpu] [--config PATH]
```

| Option | Meaning |
| --- | --- |
| `SCENE` | the `.lua` file to render |
| `--out DIR` | folder for the PNG files; created if missing |
| `--frames A..B` | only frames A to B-1, for example `--frames 0..30` for the first second |
| `--gpu` | render on the GPU (Metal or Vulkan) instead of the CPU |
| `--config PATH` | settings file; default `config/anicode.toml`, relative to the current folder |

`render` does not read Shotcut markers, so its blocks are always 2 seconds
apart.

To skip `cargo run`, build once and call the program directly:

```bash
cargo build --release -p anicode-cli
```

```bash
./target/release/anicode render examples/orbit.lua --out frames
```

## Build these docs

The docs are an [mdBook](https://rust-lang.github.io/mdBook/). The videos are
not stored in git; a script renders them from the example scenes.

```bash
brew install mdbook
```

```bash
scripts/docs-media.sh
```

```bash
mdbook serve docs --open
```

`docs-media.sh` renders every scene in `docs/src/examples/` and `examples/`
into `docs/src/media/`. Run it again after changing an example or the
engine. `mdbook serve` reloads the page when a docs file changes.
