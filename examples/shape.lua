-- A child shape moves and fades with its parent, and derive lights it up.
local box = Rect { w = 6, h = 3.5, radius = 0.3, fill = BLUE }
local level = Value(0)

-- A child of the box, so it fades in and moves with it.
local led = box:add(Circle { r = 0.18, pos = vec(0, -1.1), fill = RED })
led.opacity = derive(function() return 0.2 + 0.8 * level end)

at "intro" { fade_in(box) }
at "move" { box:move_to(vec(1, 1)), animate(level, 1) }
