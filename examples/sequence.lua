-- Unnamed play steps follow each other; a named at block can be pinned by a
-- Shotcut marker, and the steps after it follow it.
local dot = Circle { r = 0.4, x = -4, fill = BLUE }
local box = Rect { w = 1.2, h = 1.2, x = 4, fill = ORANGE }
local label = Math { "a + b = c", y = -2 }

play { fade_in(dot) }
play { shift(dot, RIGHT * 4), fade_in(box), seconds = 3 }
at "reveal" { fade_in(label) }
play { scale(box, 1.5), rotate(box, math.pi / 4) }
play { fade_out(dot):over(2) }
