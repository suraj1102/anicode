# Using Shotcut

A plugin lets Shotcut play a scene as a normal video clip with a
transparent background. Markers on the Shotcut timeline set when each `at`
block starts, so you line the animation up with a voice-over by dragging
markers, not by editing code.

## 1. Build the plugin

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
same command line program as in [Getting started](getting-started.md)).

## 2. Install the plugin

**macOS.** The normal Shotcut.app only loads plugins from inside itself.
Make a copy of the app, link the plugin into the copy, and re-sign the copy.
Run these from the repository root, and again after each Shotcut update:

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

## 3. Make a clip file for the scene

Shotcut cannot open a `.lua` file. `anicode new` writes a small `.mlt` file
next to the scene, which Shotcut can open:

```bash
./target/release/anicode new scenes/hello.lua
```

This writes `scenes/hello.mlt`. Drag it onto a video track in Shotcut.

- It needs `config/anicode.toml` in the scene's folder or a folder above it,
  and takes the frame size and rate from there.
- It will not overwrite a `.mlt` file that already exists.

## 4. Time the scene with markers

Add a Shotcut timeline marker for each `at` block, name it after the block,
and save the project.

For `hello.lua` with `at "slide" { ... }`, name the marker `slide` or
`hello:slide`. Use the `hello:` form when two scenes in one project have a
block with the same name.

- A block starts at its marker.
- A block with no marker starts 2 seconds after the one before it, so a
  scene plays even before you add any markers.
- Animations stay at the markers. If you move the clip but not the markers,
  each block still starts where its marker is.
- The engine looks for the project in the folder that holds
  `config/anicode.toml` and the folders below it, and uses the most recently
  saved project that has the scene on its timeline.

## 5. Edit and see the change

Save the `.lua` file or the Shotcut project, then step one frame or scrub.
The clip reloads. Shotcut does not redraw while paused, so the change only
shows once the playhead moves.

If the scene has a mistake, the clip keeps showing the last working version
with a red bar across the top. The message, like
`hello.lua:3: attempt to index nil`, is in Shotcut's application log. It is
quicker to read by rendering the scene with `anicode render`, which prints
it.

The clip starts out as long as the scene. You can drag its end out to make
it longer, and the last frame holds.

`examples/edit.mlt` is a small Shotcut project with markers, for trying the
marker workflow.
