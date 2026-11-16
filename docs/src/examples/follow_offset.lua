-- The label stays above the dot as it moves. Its offset is a param, so
-- dragging the label in the editor changes the offset, not the dot.
local dot = Circle { r = 0.4, pos = vec(-4, -1), fill = BLUE }
local label = Math { "x(t)", size = 0.6 }:follow(dot, param("label.offset", vec(0, 1)))

at "right" { move_to(dot, vec(4, 1)):over(2) }
