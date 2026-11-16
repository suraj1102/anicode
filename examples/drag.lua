-- Params and the editor. Open this scene with
--   cargo run --release -p anicode-editor -- examples/drag.lua
-- and drag the dot, the box or the label. The editor writes the new values
-- to drag.params.lua, and Shotcut shows them on the next frame.
local dot = Circle { r = 0.4, pos = param("dot.start", vec(-4, 0)), fill = BLUE }
local box = Rect { w = 1.2, h = 1.2, pos = param("box.pos", vec(4, 0)), fill = ORANGE }
-- The label follows the dot. Dragging it changes the offset.
local label = Math { "a + b = c" }:follow(dot, param("label.offset", vec(0, -1)))

play { fade_in(dot) }
play { move_to(dot, param("dot.end", vec(0, 1))), fade_in(box), seconds = 3 }
at "reveal" { fade_in(label) }
play { scale(box, param("box.grow", 1.5)), rotate(box, param("box.turn", math.pi / 4)) }
play { fade_out(dot):over(2), fade_out(label):over(2) }
