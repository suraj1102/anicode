-- A rotary knob. `knob.value` runs from 0 to 1 and turns the pointer.
local Knob = {}

-- Pointer angle in radians at value 0; it sweeps clockwise to -SWEEP_START.
local SWEEP_START = 2.4

function Knob.new(opts)
  local self = Group { pos = opts.pos }
  self.value = Value(opts.value or 0)
  self.body = self:add(Circle { r = 0.6, fill = opts.color })
  self.dial = self:add(Group {})
  self.dial:add(Rect { w = 0.08, h = 0.4, y = 0.3, fill = WHITE })
  self.dial.rotation = derive(function()
    return SWEEP_START - 2 * SWEEP_START * self.value
  end)
  return self
end

return Knob
