# Values and derive

## Value

`Value(start)` makes a number that is not part of any shape. You animate it
with `animate`, and shapes read it through `derive`. It lets one number
drive many shapes.

## derive

`derive(function)` makes a property that is computed again on every frame.
It is how Anicode does what Manim calls updaters.

```lua
{{#include examples/value_derive.lua}}
```

<video class="preview" src="media/value_derive.mp4" autoplay loop muted playsinline></video>

A derive can also read other shapes:

```lua
{{#include examples/follow.lua}}
```

<video class="preview" src="media/follow.mp4" autoplay loop muted playsinline></video>

For the common case of keeping one shape at a fixed offset from another,
`shape:follow(leader, offset)` writes these two derives for you. See
[Placing shapes from other shapes](params.md#placing-shapes-from-other-shapes).

## Reading numbers

A Value or a property (`level`, `box.x`) works in `+`, `-`, `*`, `/` and
unary `-`, and the result is a plain number (unless a `param` is part of it,
see [Params](params.md#doing-math-with-params)). Everything else needs the
plain number, which `:get()` returns:

```lua
level * 0.5             -- works
math.sin(level:get())   -- math functions need :get()
level:get() > 0.5       -- comparisons need :get()
```

## Rules

- A property set with `derive` cannot also be animated. The scene fails with
  `at "up" animates a value that is set with derive`.
- A derive that depends on its own result fails with
  `a derive depends on its own value`.
- A derived property can only be read while a frame renders. Reading one at
  the top level of the scene is an error.
