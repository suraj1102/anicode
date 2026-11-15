local a = Circle { r = 0.5, pos = vec(-5, 1.5), fill = BLUE }
local b = Circle { r = 0.5, pos = vec(-5, -1.5), fill = ORANGE }

-- move_to goes to a point; shift moves by an offset.
at "move" { move_to(a, vec(5, 1.5)), b:shift(RIGHT * 10) }
at "back" { move_to(a, ORIGIN + UP * 1.5), b:shift(LEFT * 5) }
