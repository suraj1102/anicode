-- param gives a value a name. A params file next to the scene can change
-- it without touching the code, and the editor changes it when you drag.
local dot = Circle { r = 0.4, pos = param("dot.start", vec(-4, 0)), fill = BLUE }

at "move" { move_to(dot, param("dot.end", vec(3, 1))):over(1.5) }
