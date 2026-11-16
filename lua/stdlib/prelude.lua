-- Loaded before every scene: 2D vectors, directions and colors.

local Vec = {}
Vec.__index = Vec

function vec(x, y)
  return setmetatable({ x = x, y = y }, Vec)
end

function Vec.__add(a, b) return vec(a.x + b.x, a.y + b.y) end
function Vec.__sub(a, b) return vec(a.x - b.x, a.y - b.y) end
function Vec.__unm(a) return vec(-a.x, -a.y) end
function Vec.__tostring(a) return "vec(" .. a.x .. ", " .. a.y .. ")" end

function Vec.__mul(a, b)
  if type(a) == "number" then return vec(a * b.x, a * b.y) end
  return vec(a.x * b, a.y * b)
end

function Vec.__div(a, b) return vec(a.x / b, a.y / b) end

ORIGIN = vec(0, 0)
LEFT = vec(-1, 0)
RIGHT = vec(1, 0)
UP = vec(0, 1)
DOWN = vec(0, -1)

-- Colors are {r, g, b, a} with channels from 0 to 1.
function rgb(hex)
  local r, g, b = hex:match("^#(%x%x)(%x%x)(%x%x)$")
  assert(r, "rgb expects a color like \"#ff8800\", got " .. tostring(hex))
  return { tonumber(r, 16) / 255, tonumber(g, 16) / 255, tonumber(b, 16) / 255, 1 }
end

WHITE = rgb("#ffffff")
BLACK = rgb("#000000")
GRAY = rgb("#888888")
RED = rgb("#fc6255")
ORANGE = rgb("#ff862f")
YELLOW = rgb("#ffe352")
GREEN = rgb("#83c167")
BLUE = rgb("#58c4dd")
PURPLE = rgb("#9a72ac")

-- Methods written in Lua that every shape has. A component library can add
-- its own: function Node.glow(self, ...) end, then box:glow(...).
Node = {}

-- Keeps this shape at `offset` (default vec(0, 0)) from `leader` on every
-- frame, as the leader moves. Both should be in the same group. With a
-- param for the offset, the editor drags the follower by changing it.
function Node.follow(self, leader, offset)
  offset = offset or ORIGIN
  self.x = derive(function() return leader.x + offset.x end)
  self.y = derive(function() return leader.y + offset.y end)
  return self
end
