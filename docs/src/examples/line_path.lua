Line { from = vec(-6, 2), to = vec(-2, 3), stroke = WHITE }

-- Path joins its points with straight lines; many points make a curve.
local wave = {}
for i = 0, 80 do
  local x = -2 + 8 * i / 80
  wave[#wave + 1] = vec(x, 2.5 + 0.6 * math.sin(2 * x))
end
Path { points = wave, stroke = GREEN, stroke_width = 0.06 }

-- closed = true joins the last point back to the first, so it can be filled.
Path { points = { vec(-4, -3), vec(-2, 0), vec(0, -3) }, closed = true, fill = ORANGE }
Path {
  points = { vec(2, -3), vec(2, -1), vec(4, 0), vec(6, -1), vec(6, -3) },
  closed = true,
  stroke = BLUE,
  stroke_width = 0.08,
}
