# Anicode Editor: Plan

Nov 16, 2026

The first prototype (commits `8796ceb` to `3a23a09`) has a preview window, scrubbing, and dragging that writes `param()` values to `<scene>.params.lua`. This plan covers how to grow it without losing what makes Anicode work: plain Lua scenes, the same frames everywhere, and Shotcut as the timeline.

## Rules every feature must keep

These are the checks for each new editor feature. If a feature breaks one, it needs a different design.

1. **The Lua file is the source of truth.** A scene must render the same with or without the editor. The editor only writes files a person could write by hand: the params file now, and small, visible edits to the scene later (phase 5).
2. **The engine decides, the editor asks.** The editor never keeps its own copy of the scene. It reloads the script and asks `Engine` questions (`display_list`, `param_for`, `params`). Any new rule (what controls a value, what a drag changes) goes in `anicode-lua` or `anicode-core`, with a test there, so the CLI and the Shotcut plugin share it.
3. **Frames stay a pure function of scene, markers, params and time.** No editor state changes a frame unless it is written to a file that the CLI and Shotcut also read.
4. **Every file stays readable.** Params, history and markers files are plain Lua or TOML, sorted and stable, so diffs in git make sense.
5. **Reload is the only update path.** A scene reload takes under 1 ms after the first one, so the editor reloads after every change. Relationships written in Lua (`label` placed from `dot`) update for free, because the script runs again.
6. **Shotcut owns audio and the edit.** The editor previews and adjusts. It never writes the Shotcut project while Shotcut may have it open.
7. **Project conventions hold.** Settings in `config/*.toml`, plain ASCII English, one change per commit, tests and clippy before each commit.

## Code layout as it grows

`crates/anicode-editor/src/main.rs` will get too long. Split it by job when each part arrives, not before:

| File | Job |
| --- | --- |
| `main.rs` | startup, config, the `eframe::App` that lays out panels |
| `preview.rs` | rendering the frame, picking, outlines, gizmos |
| `timeline.rs` | scrub bar, block ticks, later marker editing |
| `drag.rs` | cursor movement to new param values (exists) |
| `history.rs` | the undo tree, with no egui code so it is easy to test |
| `inspector.rs` | the params and properties panel |

Logic that can be a plain function (history, drag math, param formatting) stays free of egui and gets unit tests. For the window itself, add `egui_kittest` (if a 0.37 release exists) in phase 0, so tests can click and drag in a headless window. Then the editor can be checked without a person watching the screen.

## Phase 0: Fix the prototype's rough edges

Done (Nov 16, 2026). Small fixes found while building the prototype. Each one is its own commit.

- **Drag lag during an animation.** Changing a target moves the shape by the eased progress, not the full amount, so the shape trails the cursor. Divide the change by the progress when it is above a small limit (set in `config/editor.toml`). Below that limit, refuse with a message ("scrub later in the move to drag its end").
- **`pos` with `x` or `y`.** `Math { pos = ..., y = -2 }` sets y twice today, and the second one wins without a word. Reject it in `create_node` ("Math has both pos and y; use one").
- **Comments in the params file.** The editor rewrites the whole file. Change `write_params` to replace only the lines of changed params, and append new ones at the end, so comments written by hand stay.
- **Children of groups.** Dragging a shape inside a component moves only that shape. If the shape has no param, try its parents and drag the first one that has one. Say so in the status line ("moving Group on line 2").
- **Headless UI tests.** As described above.

## Phase 1: Undo and redo tree

Every change the editor makes is a change to the params file, so history is a tree of params file states.

**Model** (`history.rs`, no egui):

```rust
struct History {
    nodes: Vec<HistoryNode>,
    current: usize,
}

struct HistoryNode {
    parent: Option<usize>,
    /// The newest child is last; redo follows it.
    children: Vec<usize>,
    params: Params,
    /// Shown in the history panel, like "drag dot.start" or "edited outside".
    label: String,
    when: SystemTime,
}
```

**Rules**

- One node per gesture. A drag makes one node when it ends, not one per mouse move. A slider drag in the inspector works the same way.
- Undo moves to the parent and writes its params to the file. Redo moves to the newest child. A new change after an undo starts a new branch, and the old branch stays in the tree.
- Edits made outside the editor are kept too. When the watcher sees a params file that differs from the current node, it adds a node labeled "edited outside the editor". Undo never silently overwrites a hand edit.
- The editor's own writes are not recorded twice. After a write, the file equals the current node, so the watcher adds nothing.
- Scene code is not in this history. Your text editor and git own the `.lua` file. Phase 5 adds the scene text to the nodes once the editor can change code.

**UI**

- Cmd+Z and Cmd+Shift+Z (Ctrl on Debian).
- A history panel. It shows the tree as an indented list with the label and time of each node, and the current node marked. Clicking a node jumps to that state, so you can compare two branches by clicking between them.

**Saving the history.** Start with history in memory only. Then save it to `.anicode/history/<scene>.toml` under the project root (gitignored) after each change. Reopening the editor then keeps the tree. The number of nodes kept goes in `config/editor.toml`.

**Tests:** undo then a new edit makes a branch, and redo follows the newest child. Outside edits become nodes. A write followed by a reload adds no node. A saved history loads back the same.

## Phase 2: Placing objects from other objects

Your example:

```lua
local label = Math { "a + b = c", pos = dot:pos() + vec(-1, 0) }
```

**What happens today.** This already renders. `dot:pos()` returns the dot's base position when the script runs, so the label is placed once, 1 unit left of where the dot starts. Dragging the dot changes `dot.start`, the script runs again, and the label moves with it. The label itself can't be dragged, because the sum is a plain vec with no param in it. (Also note your example needs a comma, `vec(-1, 0)`, and the `y = -2` would replace the y from `pos`, see phase 0.)

**What to add: tracking through arithmetic.** Today a value remembers its param only if it is the param itself. Extend that to sums and constant factors. Each number carries `(param, part, gain)`, meaning "this value is a constant plus `gain` times that param". Then a drag by `d` changes the param by `d / gain`.

| Expression | Tracked as |
| --- | --- |
| `param("p", 1)` | `p`, gain 1 |
| `p + 2`, `p - 2`, `2 + p` | `p`, gain 1 |
| `p * 3`, `p / 2`, `-p` | `p`, gain 3, 0.5, -1 |
| `p + q` (two params) | untracked, the message names both |
| `p * q`, `math.sin(p)` | untracked |

Vec arithmetic lives in `lua/stdlib/prelude.lua`, so vec tracking is added there per component. Number tracking stays in the `ParamNumber` metamethods in `bindings.rs`.

**`dot:pos()` should carry the dot's tracking.** If the dot's x comes from `dot.start`, then `dot:pos()` is tracked to `dot.start`. That raises the real design question. In

```lua
local label = Math { "a + b = c", pos = dot:pos() + param("label.offset", vec(-1, 0)) }
```

the label depends on two params. Dragging the label should change `label.offset`, not move the dot. The rule:

- A param written in the node's own expression wins over one that comes in through another node.
- If the only param comes from another node (`pos = dot:pos() + vec(-1, 0)`), don't guess. Show "label is placed from dot on line 3; drag dot, or make the offset a param", and maybe offer to drag the dot with a modifier key.

This is a change to the "Record where each param is used" data in `anicode-core`: `ParamUse` gets a `gain` and a "direct or through another node" flag. Tests in `anicode-lua`: each row of the table above, and both label cases.

## Phase 3: Relationships that follow over time

`dot:pos()` is read once, when the script runs. If the dot moves during the scene, a label placed from it stays behind. To follow it, the scene needs a value computed every frame:

```lua
label:follow(dot, param("label.offset", vec(-1, 0)))
-- the same as
label.x = derive(function() return dot.x + offset.x end)
label.y = derive(function() return dot.y + offset.y end)
```

- Add `follow` (and later `next_to`, `below` with a gap) as Lua components in the stdlib, built on `derive`. They need no Rust, so they keep the "components are just Lua" rule.
- Dragging a followed object: run its derive functions with tracked inputs, using the same gain tracking as phase 2. Then a drag changes the offset param. Until that works, `param_for` already reports "computed by derive", which is correct.

## Phase 4: Inspector panel

A side panel with two parts:

- **Params.** Every param the scene uses, with its value, and where it comes from (file or default). Numbers get a drag field and vecs get two. Changes write the params file and add history nodes like a drag does. A "reset to default" button removes the param from the file.
- **Selected object.** Its label ("Circle on line 3"), its properties at the playhead, and what sets each one: param, code (with the line), or derive. A button opens the scene in your editor at that line. The command goes in `config/editor.toml`, for example `code -g {file}:{line}` or `nvim +{line} {file}`.

This makes "why can't I drag this" visible without trial and error.

## Phase 5: Turning code into params

When a value is a number in the code, the editor could offer to make it a param:

```lua
local dot = Circle { r = 0.4, x = -4 }            -- before
local dot = Circle { r = 0.4, x = param("dot.x", -4) }  -- after one click
```

- This is the first time the editor changes the `.lua` file, so it has to be careful. It only rewrites the single number it found, on the line the engine recorded, and only if that line still matches. Otherwise it refuses.
- The engine records the line today but not the column. Record the source span of each option value (by finding the key in that line's text) when it is set.
- From this phase on, history nodes hold the scene text too, so undo covers code edits.
- If your text editor has unsaved changes to the scene, a write would clash. The editor checks the file's modified time right before it writes, and refuses if the file changed since the last reload.

## Phase 6: Timeline editing

The tick marks are read-only now. Retiming belongs in Shotcut, against the voice-over. But scenes without a project (or before the voice-over exists) need a way too:

- Dragging a tick writes a `<scene>.markers.lua` file, the "markers edited by hand" fallback from the build plan. The engine reads it like the params file, and Shotcut markers win when both exist.
- With a Shotcut project, ticks show the project's markers, and a tick can't be dragged while Shotcut has the project open.
- Show each animation as a bar under its block, so you can see where `:over(2)` ends.

## Phase 7: GPU preview

Render with Vello straight into egui's wgpu device (both use wgpu 30). That gives full resolution at any window size, and drops the CPU copy. Keep the CPU renderer as the fallback, and for tests.

## Phase 8: Gizmos and snapping

- Handles on the selected object for scale and rotation, instead of Shift and Alt.
- Snapping to the grid, to the frame center and edges, and to other objects. Grid size goes in config.
- Selecting more than one object, and dragging them together.

## Suggested order

| Order | Phase | Why then |
| --- | --- | --- |
| 1 | 0: rough edges | small, and makes the rest easier to test |
| 2 | 1: undo tree | needed before drags get more powerful |
| 3 | 2: tracking through arithmetic | your label example |
| 4 | 4: inspector | makes phases 2 and 3 visible and debuggable |
| 5 | 3: follow over time | builds on 2 |
| 6 | 5: code into params | the riskiest, needs history and the inspector first |
| 7 | 6, 7, 8 | when the daily loop asks for them |

## Open questions

1. When the only param behind an object comes from another object (`pos = dot:pos() + vec(-1, 0)`), should dragging it refuse with a message, or move the other object?
2. Should the history be saved to disk from the start, or stay in memory until the tree UI settles?
3. In phase 5, may the editor change the `.lua` file at all, or should code stay hand-written only?
