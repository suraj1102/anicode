-- The four easing curves, :over, chained moves and fade_out.
local easings = { "linear", "in", "out", "inout" }
local colors = { BLUE, GREEN, YELLOW, RED }

local dots = {}
for i, easing in ipairs(easings) do
  local y = 2.25 - (i - 1) * 1.5
  Line { from = vec(-5, y), to = vec(5, y), stroke = GRAY, stroke_width = 0.02 }
  dots[i] = Circle { r = 0.3, pos = vec(-5, y), fill = colors[i] }
end

-- Loops cannot go inside the { } of an at block, so they live in functions
-- that the block calls.
local function show_all()
  for _, dot in ipairs(dots) do fade_in(dot):over(0.5) end
end

local function race()
  for i, dot in ipairs(dots) do move_to(dot, vec(5, dot.y)):ease(easings[i]) end
end

local function come_back()
  for _, dot in ipairs(dots) do
    -- Methods animate inside a block too, but only the global form returns
    -- something that takes :over and :ease.
    dot:scale(0.5)
    move_to(dot, vec(0, dot.y)):over(1)
  end
end

local function hide_all()
  for _, dot in ipairs(dots) do fade_out(dot) end
end

at "show" { show_all() }
at "race" { race() }
at "back" { come_back() }
at "hide" { hide_all() }
