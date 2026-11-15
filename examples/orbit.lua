-- derive: positions computed every frame from one animated angle, and a
-- moon that orbits inside the planet's group.
local angle = Value(0)

local sun = Circle { r = 0.8, fill = YELLOW }
-- Lua's math functions need plain numbers, so read the value with :get().
sun.r = derive(function() return 0.8 + 0.05 * math.sin(4 * angle:get()) end)

Circle { r = 3, stroke = GRAY, stroke_width = 0.02 }

local planet = Group {}
planet.x = derive(function() return 3 * math.cos(angle:get()) end)
planet.y = derive(function() return 3 * math.sin(angle:get()) end)
planet:add(Circle { r = 0.35, fill = BLUE })

-- Children use the parent's position, so the moon only needs its offset.
local moon = planet:add(Circle { r = 0.12, fill = WHITE })
moon.x = derive(function() return 0.7 * math.cos(5 * angle:get()) end)
moon.y = derive(function() return 0.7 * math.sin(5 * angle:get()) end)

at "appear" { fade_in(sun), fade_in(planet) }
at "orbit" { animate(angle, 2 * math.pi):over(2):ease("linear") }
at "fast" { animate(angle, 6 * math.pi):over(2):ease("in") }
