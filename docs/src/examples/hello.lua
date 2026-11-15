local box = Rect { w = 3, h = 2, radius = 0.2, fill = BLUE }

at "intro" { fade_in(box) }
at "slide" { move_to(box, vec(3, 1)):over(1):ease("out") }
at "leave" { fade_out(box) }
