-- Tex takes LaTeX math. [[ ]] strings keep the backslashes as they are.
local integral = Tex { [[\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}]], y = 1.5, fill = GREEN }
local matrix = Tex { [[A = \begin{pmatrix} a & b \\ c & d \end{pmatrix}]], y = -1.5, size = 0.6, fill = ORANGE }

at "show" { fade_in(integral), fade_in(matrix) }
at "turn" { matrix:rotate(0.2) }
