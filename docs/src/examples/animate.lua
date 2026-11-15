local box = Rect { w = 1, h = 1, x = -3, fill = BLUE }
local dot = Circle { r = 0.3, x = 3, fill = RED }

-- animate tweens any property: here width, height, corner radius and radius.
at "stretch" { animate(box.w, 4), animate(dot.r, 1.5) }
at "round" { animate(box.h, 3), animate(box.radius, 0.5) }
at "thin" { animate(dot.opacity, 0.3) }
