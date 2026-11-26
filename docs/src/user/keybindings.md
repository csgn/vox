# Keybindings

Not configurable yet (planned for M3).

## Grammar

```
["x][count] operator [count] motion     e.g. d2w, "+yy, 3dd
["x][count] motion                      e.g. 5j, 3w, 10G
["x][count] action                      e.g. 3x, "+p, 2u
```
Counts multiply: `2d3w` deletes 6 words. `Esc` cancels a pending command.

## Motions

| Key | Motion | Kind |
|---|---|---|
| `h`, `Left` / `l`, `Right`, `Space` | left / right | exclusive |
| `j`, `Down` / `k`, `Up` | down / up (keeps column) | linewise |
| `w` | start of next word | exclusive |
| `b` | start of current/previous word | exclusive |
| `e` | end of current/next word | inclusive |
| `0`, `Home` | start of line | exclusive |
| `^` | first non-blank | exclusive |
| `$`, `End` | end of line (`N$`: N-1 lines down) | inclusive |
| `gg` / `G` | first / last line; `Ngg`, `NG`: line N | linewise |
| `n` / `N` | next / previous match of last search | exclusive |

Words: runs of letters/digits/`_`, or runs of other punctuation. Empty lines count as words.

## Operators

| Key | Operator | Doubled |
|---|---|---|
| `d` | delete | `dd` deletes lines |
| `c` | change (delete, then insert) | `cc` changes lines |
| `y` | yank (copy) | `yy` yanks lines |

Vim rules kept: `dw` on the last word of a line stops at the line end; `cw` on a word acts like `ce`.

## Other normal-mode keys

| Key | Action |
|---|---|
| `x` | delete char (same as `dl`) |
| `p` / `P` | paste after / before (linewise text: below / above the line) |
| `i` / `a` | insert before / after cursor |
| `I` / `A` | insert at first non-blank / end of line |
| `o` / `O` | open line below / above |
| `u` / `Ctrl-r` | undo / redo |
| `v` / `V` | visual charwise / linewise |
| `/` | search |
| `:` | command line |
| `Ctrl-d` / `Ctrl-u` | half page down / up |
| `Ctrl-f` / `Ctrl-b`, `PageDown` / `PageUp` | page down / up |

## Registers

| Register | Meaning |
|---|---|
| (none) | unnamed register: every delete, change, and yank writes it |
| `"+`, `"*` | system clipboard (both the same) |

Yanks and deletes do not touch the system clipboard unless you prefix `"+`.
Clipboard text that ends with a newline pastes linewise.
Named registers (`"a`–`"z`) are not supported.

## Visual mode

`v` (charwise) or `V` (linewise) starts a selection at the cursor. Motions extend it.
`d`/`x` delete, `c` changes, `y` yanks the selection. `v`/`V` switch type or exit; `Esc` exits.

## Search

`/text<Enter>` jumps to the next occurrence. Literal, case-sensitive text (no regex).
`/<Enter>` repeats the last search. `n`/`N` repeat forward/backward; searches wrap around.

## Insert mode

| Key | Action |
|---|---|
| any char, `Tab` | insert |
| `Enter` | new line |
| `Backspace` | delete before cursor; at column 0 joins with previous line |
| arrows | move |
| `Esc` | back to normal mode (cursor moves one left) |

One insert session (including the delete of `c`) is one undo step.

## Command line and search prompt

| Key | Action |
|---|---|
| `Enter` | execute |
| `Esc` | cancel |
| `Backspace` | delete char; on empty input, cancel |

## Not supported (by design, for now)
Dot-repeat (`.`), macros, marks, named registers, text objects (`iw`, `i(`…), block visual,
`f`/`t`, regex search, `:s`.
