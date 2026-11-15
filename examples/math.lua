-- Typst and LaTeX math.
local title = Math { "y(t) = tanh(g dot x(t))", size = 0.7, y = 2.6 }
local fraction = Math { "f(x) = (a + b) / (2 c)", x = -3.2, y = 0.6, fill = ORANGE }
local integral = Tex { [[\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}]], x = 3, y = 0.6, fill = BLUE }
local matrix = Tex { [[A = \begin{pmatrix} a & b \\ c & d \end{pmatrix}]], x = -3.2, y = -2, fill = GREEN }
local sum = Math { "sum_(n=1)^oo 1 / n^2 = pi^2 / 6", x = 3, y = -2, fill = PURPLE }

at "title" { fade_in(title) }
at "typst" { fade_in(fraction), fade_in(sum) }
at "latex" { fade_in(integral), fade_in(matrix) }
at "zoom" { title:scale(1.3) }
