-- A vertical level meter. `meter.level` runs from 0 to 1 and lights the
-- segments from the bottom up.
local Meter = {}

local SEGMENTS = 10
local SEGMENT_HEIGHT = 0.3
local GAP = 0.06

function Meter.new(opts)
  local self = Group { pos = opts.pos }
  self.level = Value(opts.level or 0)
  for i = 1, SEGMENTS do
    local color = GREEN
    if i > 8 then color = RED elseif i > 6 then color = YELLOW end
    local segment = self:add(Rect {
      w = 0.6,
      h = SEGMENT_HEIGHT,
      y = (i - 1) * (SEGMENT_HEIGHT + GAP),
      fill = color,
    })
    -- Comparisons need plain numbers, so read the value with :get().
    segment.opacity = derive(function()
      if self.level:get() >= i / SEGMENTS then return 1 end
      return 0.15
    end)
  end
  return self
end

return Meter
