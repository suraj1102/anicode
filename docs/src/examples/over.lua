local seconds = { 0.5, 1, 2, 3 }
local dots = {}
for i, length in ipairs(seconds) do
  dots[i] = Circle { r = 0.35, pos = vec(-5, 2.25 - (i - 1) * 1.5), fill = BLUE }
end

local function go()
  for i, dot in ipairs(dots) do
    move_to(dot, vec(5, dot.y)):over(seconds[i]):ease("linear")
  end
end

at "go" { go() }
