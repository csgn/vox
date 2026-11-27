# Keybindings

These are the built-in keys. Any of them can be remapped or disabled in the config file:
see [Configuration](configuration.md).

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
| `<leader>f` / `<leader>F` | [focus](focus.md) a larger / smaller unit (block, function, class, file) |
| `Esc` | end focus (after leaving visual mode or a pending command) |
| `<leader>m` | switch between [text and agent mode](agent-mode.md) |
| `<leader>a` | agent mode: [ask the AI](agent-mode.md#asking) about the target |
| `ga` / `gx`, `gA` / `gX`, `]c` / `[c` | agent mode: accept / decline the AI change (all), next / previous change |
| `<leader>p` | plugin manager |
| `<C-w>h/j/k/l/w`, `<C-w>c` | move focus between text and panels, close a panel (see [Panels](interface.md)) |
| `Ctrl-d` / `Ctrl-u` | half page down / up |
| `Ctrl-f` / `Ctrl-b`, `PageDown` / `PageUp` | page down / up |

## Key hints

Press the start of a mapping (`<leader>`, or `g`, `]` when mappings start with them) and
pause: after `ui.key_hints_delay_ms` (300 ms) a box lists the keys that can follow and what
each runs, e.g. `p  :plugins`; `+3` marks a group of longer mappings (`<leader>d` in
[developer mode](developer-mode.md)). While the box is open vox waits for the next key
instead of timing out; a group key shows its own keys at once. `Esc` closes the box and
drops the typed keys. Typing faster than the delay never shows it. Normal and visual mode
only. Where it shows: `ui.key_hints = "bottom-right"` (default), `"modal"` (centered), or
`"off"` ([Interface settings](interface.md#settings)).

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
| `Esc` | cancel; with a completion selected, back to what you typed |
| `Backspace` | delete char; on empty input, cancel |
| `Tab` | complete (`:` only): the only match, else the common part, else select the first |
| `Shift-Tab` | select the previous completion |
| `Up` / `Down`, `<C-p>` / `<C-n>` | move in the completion menu; without a menu, walk the history |

See [Typing commands](commands.md#typing-commands) for completion and where the prompt is
shown.

## Not supported (by design, for now)
Dot-repeat (`.`), macros, marks, named registers, text objects (`iw`, `i(`…), block visual,
`f`/`t`, regex search, `:s`.
