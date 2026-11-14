-- Shapes, groups, a Lua component, at blocks, animate and derive.
local Knob = require "knob"

local panel = Rect { w = 6, h = 3.5, radius = 0.3, fill = rgb("#2b2d31") }
local gain = Knob.new { pos = vec(-1.5, 0.3), color = ORANGE }
local tone = Knob.new { pos = vec(1.5, 0.3), color = BLUE, value = 0.5 }

-- A child of the panel, so it fades in with it.
local led = panel:add(Circle { r = 0.18, pos = vec(0, -1.1), fill = RED })
led.opacity = derive(function() return 0.2 + 0.8 * gain.value end)

local points = {}
for i = 0, 60 do
  local x = -2.5 + 5 * i / 60
  points[#points + 1] = vec(x, 0.4 * math.sin(3 * x))
end
local wave = Path { points = points, stroke = GREEN, stroke_width = 0.05, y = -2.7 }
local baseline = Line { from = vec(-3, -3.3), to = vec(3, -3.3), stroke = GRAY }

at "intro" { fade_in(panel), fade_in(gain), fade_in(tone) }
at "turn_gain" { animate(gain.value, 1):over(1.5) }
at "turn_tone" { animate(tone.value, 0.1):ease("out"), fade_in(wave), fade_in(baseline) }
at "zoom" { gain:scale(1.4):shift(LEFT * 0.5) }
