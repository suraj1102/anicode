-- The frame is 8 units tall. The origin is the center and y points up.
Line { from = vec(-7, 0), to = vec(7, 0), stroke = GRAY }
Line { from = vec(0, -4), to = vec(0, 4), stroke = GRAY }

local points = { vec(0, 0), vec(3, 2), vec(-4, -2), vec(4.5, -3) }
for _, point in ipairs(points) do
  Circle { r = 0.12, pos = point, fill = YELLOW }
  Math { "(" .. point.x .. ", " .. point.y .. ")", size = 0.35, pos = point + vec(0.7, 0.35) }
end
