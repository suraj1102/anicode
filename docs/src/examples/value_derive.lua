-- One Value drives three shapes through derive.
local level = Value(0)

-- The bar grows upward: its center moves up by half of its height.
local bar = Rect { w = 0.6, x = -4, fill = GREEN }
bar.h = derive(function() return 0.2 + 5 * level end)
bar.y = derive(function() return -2.5 + bar.h:get() / 2 end)

local lamp = Circle { r = 0.8, fill = YELLOW }
lamp.opacity = derive(function() return 0.1 + 0.9 * level end)

-- Lua's math functions need a plain number, so read it with :get().
local needle = Rect { w = 0.1, h = 2, x = 4, fill = RED }
needle.rotation = derive(function() return math.pi / 2 * math.cos(math.pi * level:get()) end)

at "up" { animate(level, 1):over(2) }
at "down" { animate(level, 0.2) }
