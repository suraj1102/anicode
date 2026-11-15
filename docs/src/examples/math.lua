-- Math uses Typst math syntax.
local euler = Math { "e^(i pi) + 1 = 0", size = 0.8, y = 2.3 }
local quadratic = Math { "x = (-b plus.minus sqrt(b^2 - 4 a c)) / (2 a)", y = 0.2, fill = YELLOW }
local sum = Math { "sum_(n=1)^oo 1 / n^2 = pi^2 / 6", y = -2, fill = BLUE }

at "one" { fade_in(euler) }
at "two" { fade_in(quadratic), fade_in(sum) }
at "zoom" { euler:scale(1.4) }
