local left = Circle { r = 1, x = -3, fill = BLUE }
local right = Rect { w = 2, h = 2, x = 3, fill = ORANGE }

-- fade_in hides the shape until its block starts.
at "show" { fade_in(left) }
at "both" { fade_in(right) }
at "hide" { fade_out(left), fade_out(right) }
