// Typst definitions for the names that mitex's LaTeX conversion produces
// but Typst does not have. The engine draws each formula in one color, so
// \color is accepted but has no effect.

#let zws = sym.zws

// \sqrt[n]{x} arrives as mitexsqrt([n], x), with the brackets in the content.
#let mitexsqrt(..args) = {
  let args = args.pos()
  if args.len() == 1 {
    math.sqrt(args.at(0))
  } else {
    math.root(args.at(0).children.slice(1, -1).join(), args.at(1))
  }
}

#let mitexmathbf(body) = math.upright(math.bold(body))
#let mitexbold(body) = math.bold(body)
#let mitexitalic(body) = math.italic(body)
#let mitexupright(body) = math.upright(body)
#let mitexsans(body) = math.sans(body)
#let mitexmono(body) = math.mono(body)
#let mitexcal(body) = math.cal(body)
#let mitexfrak(body) = math.frak(body)

#let mitexdisplay(body) = math.display(body)
#let mitexinline(body) = math.inline(body)
#let mitexscript(body) = math.script(body)
#let mitexsscript(body) = math.sscript(body)

// The annotation follows as ^(...) or _(...), so it must sit above or below.
#let mitexoverbrace(body) = math.limits(math.overbrace(body))
#let mitexunderbrace(body) = math.limits(math.underbrace(body))
#let mitexoverbracket(body) = math.limits(math.overbracket(body))
#let mitexunderbracket(body) = math.limits(math.underbracket(body))

#let mitexcolor(color, ..body) = body.pos().join()
#let mitexlabel(..args) = none
#let mitexcaption(..args) = none

#let textmath(body) = body
#let operatorname(body) = math.op(math.upright(body))
#let aligned(body) = body

#let matrix(..rows) = math.mat(delim: none, ..rows)
#let pmatrix(..rows) = math.mat(delim: "(", ..rows)
#let bmatrix(..rows) = math.mat(delim: "[", ..rows)
#let Bmatrix(..rows) = math.mat(delim: "{", ..rows)
#let vmatrix(..rows) = math.mat(delim: "|", ..rows)
#let Vmatrix(..rows) = math.mat(delim: "||", ..rows)
#let smallmatrix(..rows) = math.script(math.mat(delim: none, ..rows))
// Column alignment from \begin{array}{...} is not applied.
#let mitexarray(arg0: none, ..rows) = math.mat(delim: none, ..rows)
