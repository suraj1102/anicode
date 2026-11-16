# Anicode

Anicode turns Lua scene files into animations. You can use it on its own,
where `anicode render` writes PNG frames, or inside Shotcut, where a plugin
plays a scene as a clip and Shotcut timeline markers set its timing.

## Docs

The docs show every example next to its rendered video. They are online at
https://suraj1102.github.io/anicode/ and rebuild on every push to `main`. To
open them locally:

```bash
brew install mdbook git-lfs
```

```bash
mdbook serve docs --open
```

The pages are Markdown files in [docs/src](docs/src), and the example scenes
are in [docs/src/examples](docs/src/examples). The videos in `docs/src/media`
are stored in Git LFS and are not rendered by CI. After changing a scene,
render them again and commit the result:

```bash
scripts/docs-media.sh
```

The design and roadmap are in [anicode-build-plan.md](anicode-build-plan.md),
and the editor's plan is in [anicode-editor-plan.md](anicode-editor-plan.md).

## Quick start

Run from the repository root.

```bash
cargo run --release -p anicode-cli -- render examples/orbit.lua --out frames
```

```bash
ffmpeg -y -framerate 29.97 -i frames/frame_%05d.png -pix_fmt yuv420p orbit.mp4 && open orbit.mp4
```

The first command writes `frames/frame_00000.png` and so on, then prints one
line with the frame count. Nothing plays until you make the video.

- Always pass `--release`. A debug build is about 100 times slower and looks
  stuck.
- `render` never deletes old frames. Run `rm -rf frames` before rendering a
  different scene.

To play a scene in a window instead, and drag shapes to change their params:

```bash
cargo run --release -p anicode-editor -- examples/drag.lua
```

## What works today

- Shapes: `Rect`, `Circle`, `Line`, `Path`, `Group`, and math with `Math`
  (Typst) and `Tex` (LaTeX).
- Animations: `at` blocks, `animate`, `fade_in`, `fade_out`, `move_to`,
  `:over`, `:ease`, and the `shift`, `scale` and `rotate` methods.
- `Value` and `derive` for numbers computed every frame, and `follow` to
  keep one shape next to another.
- `param("name", default)`, with values in a `scene.params.lua` file next to
  the scene.
- Components in plain Lua, loaded with `require`.
- `anicode render` (PNG frames) and `anicode new` (Shotcut clip file).
- The Shotcut plugin: plays scenes, reloads when the scene or its params
  file is saved, reads marker times, shows a red bar on script errors.
- `anicode-editor`: a preview window with play, scrub and block ticks.
  Dragging a shape changes the param behind it. It has an inspector for
  params and the selected shape, and an undo tree that is saved between
  sessions.

## Tests

```bash
cargo test --workspace
```

This runs the unit tests and the golden-frame test, which compares the
scenes listed under `[golden]` in `config/anicode.toml` with the PNGs in
`tests/golden/`. To save new reference frames after an intended change:

```bash
ANICODE_UPDATE_GOLDEN=1 cargo test -p anicode-cli --test golden
```

After `cmake --build build`, check that every example plays in `melt`:

```bash
scripts/melt-smoke.sh
```
