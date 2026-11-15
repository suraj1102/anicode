-- Children move, turn, scale and fade with their group.
local robot = Group { x = -3 }
robot:add(Rect { w = 2, h = 2.4, radius = 0.2, fill = GRAY })
robot:add(Circle { r = 0.25, pos = vec(-0.45, 0.5), fill = BLUE })
robot:add(Circle { r = 0.25, pos = vec(0.45, 0.5), fill = BLUE })
robot:add(Rect { w = 1, h = 0.15, y = -0.5, fill = WHITE })

at "turn" { robot:rotate(math.pi / 6) }
at "walk" { robot:shift(RIGHT * 6) }
at "grow" { robot:scale(1.5) }
at "leave" { fade_out(robot) }
