# Focus mode

Focus on one unit of code — a selection, block, function, class, or the whole file — while
you work on it. The rest of the file is dimmed, and the status line shows what is focused:
`FOCUS function area (L4–10)`. In [agent mode](agent-mode.md) the focused unit is also what
the AI looks at and may change.

## Choosing the unit

| Key / command | Action |
|---|---|
| `<leader>f` (`:narrow out`) | focus the smallest unit around the cursor; again: the next larger one |
| `<leader>F` (`:narrow in`) | the next smaller unit at the cursor, inside the focused one |
| `<leader>f` in visual mode (`:narrow selection`) | focus the selected lines |
| `:narrow block\|function\|class\|file` | the nearest unit of that kind around the cursor |
| `:narrow` | the smallest unit around the cursor |
| `Esc` (normal mode), `:widen` | end focus |

`<leader>` is `\` unless you changed it. Widening goes block → function → class → file;
narrowing goes back.

You can move and edit anywhere; the focused range follows edits (lines added inside it make
it longer, lines added above move it down). Switching to another buffer ends focus.

## How units are found

vox needs no language support for this. It looks at:

- **braces** `{ }` — strings and comments are skipped, and a signature spread over several
  lines (`fn f(\n  a,\n) -> T {`) belongs to its function;
- **indentation** — a line ending with `:` and the more-indented lines after it (Python,
  YAML-like files);
- **the first word** of the unit's first line: `fn`, `def`, `function`, `func`, `fun` make a
  function; `class`, `struct`, `impl`, `trait`, `interface`, `enum`, `mod`, `module` make a
  class; `name(...) {` (C, Java, JavaScript methods) is a function. Anything else is a block.

This works for most code; when it guesses wrong, select the lines in visual mode and use
`<leader>f`.
