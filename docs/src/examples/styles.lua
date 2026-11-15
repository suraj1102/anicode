local colors = { RED, ORANGE, YELLOW, GREEN, BLUE, PURPLE, GRAY, WHITE }
for i, color in ipairs(colors) do
  Circle { r = 0.6, x = -6.3 + (i - 1) * 1.8, y = 2.2, fill = color }
end

-- rgb() reads a hex color. A fourth value in a color table is its alpha.
Rect { w = 3, h = 2, x = -4, y = -1.5, fill = rgb("#3b82f6") }
Rect { w = 3, h = 2, x = -2.5, y = -2.2, fill = { 1, 0.5, 0, 0.6 } }

-- opacity fades the whole shape, fill and stroke together.
for i = 1, 5 do
  Circle { r = 0.6, x = 0.8 + (i - 1) * 1.4, y = -1.8, fill = BLUE, stroke = WHITE, opacity = i / 5 }
end
