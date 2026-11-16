-- sequence.lua with params. Open it in anicode-editor and drag the dot or
-- the box: the editor writes the new values to drag.params.lua.
local dot = Circle { r = 0.4, pos = param("dot.start", vec(-4, 0)), fill = BLUE }
local box = Rect { w = 1.2, h = 1.2, pos = param("box.pos", vec(4, 0)), fill = ORANGE }
local label = Math { "a + b = c", y = -2 }

play { fade_in(dot) }
play { move_to(dot, param("dot.end", vec(0, 1))), fade_in(box), seconds = 3 }
at "reveal" { fade_in(label) }
play { scale(box, param("box.grow", 1.5)), rotate(box, param("box.turn", math.pi / 4)) }
play { fade_out(dot):over(2) }
