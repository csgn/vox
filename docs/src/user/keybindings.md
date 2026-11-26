# Keybindings

Not configurable yet (planned for M3). No counts or operators yet (planned for M2).

## Normal mode

| Key | Action |
|---|---|
| `h`, `Left` / `l`, `Right` | left / right |
| `j`, `Down` / `k`, `Up` | down / up (keeps column) |
| `w` | start of next word |
| `b` | start of current/previous word |
| `0`, `Home` | start of line |
| `$`, `End` | end of line |
| `gg` / `G` | first / last line (first non-blank) |
| `Ctrl-d` / `Ctrl-u` | half page down / up |
| `Ctrl-f` / `Ctrl-b`, `PageDown` / `PageUp` | page down / up |
| `i` / `a` | insert before / after cursor |
| `o` / `O` | open line below / above |
| `x` | delete char under cursor |
| `u` / `Ctrl-r` | undo / redo |
| `:` | command line |

Words: runs of letters/digits/`_`, or runs of other punctuation. Empty lines count as words.

## Insert mode

| Key | Action |
|---|---|
| any char, `Tab` | insert |
| `Enter` | new line |
| `Backspace` | delete before cursor; at column 0 joins with previous line |
| arrows | move |
| `Esc` | back to normal mode (cursor moves one left) |

One insert session (from entering insert mode to `Esc`) is one undo step.

## Command line

| Key | Action |
|---|---|
| `Enter` | execute |
| `Esc` | cancel |
| `Backspace` | delete char; on empty input, cancel |
