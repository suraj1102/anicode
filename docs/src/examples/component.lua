-- A component is a function that builds shapes in a Group and returns it.
local function ProgressBar(opts)
  local self = Group { pos = opts.pos }
  self.progress = Value(0)
  self:add(Rect { w = 6, h = 0.5, radius = 0.25, stroke = GRAY })
  local fill = self:add(Rect { h = 0.4, radius = 0.2, fill = opts.color })
  fill.w = derive(function() return 0.4 + 5.5 * self.progress end)
  -- Keep the left edge in place while the width grows.
  fill.x = derive(function() return -2.95 + fill.w:get() / 2 end)
  return self
end

local download = ProgressBar { pos = vec(0, 1), color = BLUE }
local upload = ProgressBar { pos = vec(0, -1), color = GREEN }

at "start" { animate(download.progress, 0.6):over(1.5), animate(upload.progress, 0.3):over(1.5) }
at "finish" { animate(download.progress, 1), animate(upload.progress, 1):over(2) }
