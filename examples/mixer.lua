-- A component from examples/lib, driven by animate on its Value.
local Meter = require "meter"

local meters = {}
for i = 1, 4 do
  meters[i] = Meter.new { pos = vec(-3 + (i - 1) * 2, -1.8) }
end

local function show_all()
  for _, meter in ipairs(meters) do fade_in(meter) end
end

local function set_levels(levels)
  for i, meter in ipairs(meters) do
    animate(meter.level, levels[i]):over(0.6):ease("out")
  end
end

at "show" { show_all() }
at "verse" { set_levels { 0.4, 0.6, 0.5, 0.3 } }
at "chorus" { set_levels { 0.8, 1.0, 0.9, 0.7 } }
at "quiet" { set_levels { 0.1, 0.2, 0.1, 0 } }
