local easings = { "linear", "in", "out", "inout" }
local dots = {}
for i, easing in ipairs(easings) do
  local y = 2.25 - (i - 1) * 1.5
  Math { '"' .. easing .. '"', size = 0.4, pos = vec(-6, y) }
  dots[i] = Circle { r = 0.3, pos = vec(-4.5, y), fill = YELLOW }
end

local function race()
  for i, dot in ipairs(dots) do
    move_to(dot, vec(5, dot.y)):over(2):ease(easings[i])
  end
end

at "race" { race() }
