-- Five steps: four unnamed play steps and one named at block.
local title = Math { "x^2 + y^2 = r^2", size = 0.8, y = 2.5 }
local circle = Circle { r = 1, stroke = BLUE, stroke_width = 0.06 }
local dot = Circle { r = 0.12, x = 1, fill = ORANGE }
local radius = Line { from = vec(0, 0), to = vec(1, 0), stroke = ORANGE }

play { fade_in(title) }                       -- steps:1
play { fade_in(circle), fade_in(radius) }     -- steps:2
play { fade_in(dot), seconds = 3 }            -- steps:3, holds 3 s
at "spin" { rotate(radius, math.pi), move_to(dot, vec(-1, 0)) }
play { scale(circle, 1.5), fade_out(title):over(2) }   -- steps:4