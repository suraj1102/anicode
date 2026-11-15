local a = Circle { r = 0.4, pos = vec(-5, 1.5), fill = BLUE }
local b = Circle { r = 0.4, pos = vec(-5, -1.5), fill = ORANGE }

-- Blocks start 2 seconds apart. This 3 second move is still running when
-- the next block starts.
at "first" { move_to(a, vec(5, 1.5)):over(3):ease("linear") }
at "second" { move_to(b, vec(5, -1.5)):ease("linear") }
