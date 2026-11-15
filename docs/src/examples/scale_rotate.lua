local square = Rect { w = 2, h = 2, x = -3, fill = GREEN }
local bar = Rect { w = 3, h = 0.4, x = 3, fill = PURPLE }

at "grow" { square:scale(1.5), bar:rotate(math.pi / 2) }
at "shrink" { square:scale(0.5), bar:rotate(-math.pi) }
