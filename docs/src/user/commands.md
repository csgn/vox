# Commands

| Command | Action |
|---|---|
| `:w` | write to the current file |
| `:w FILE` | write to `FILE` and make it the current file |
| `:q`, `:quit`, `:qa` | quit; refuses with `E37` if there are unsaved changes |
| `:q!` | quit, discarding changes |
| `:wq [FILE]` | write, then quit |
| `:x [FILE]` | write if modified, then quit |
| `:e FILE` | open `FILE`; refuses with `E37` if there are unsaved changes |
| `:e` | reload the current file |
| `:e! [FILE]` | open/reload, discarding changes |

## Messages
| Message | Meaning |
|---|---|
| `"f" NL, NB written` | write succeeded |
| `E32: No file name` | `:w` on an unnamed buffer |
| `E37: No write since last change` | unsaved changes; add `!` to override |
| `E212: Can't open file for writing` | write failed |
| `E484: Can't open file` | `:e` failed |
| `E492: Not an editor command` | unknown command |

Note: unlike Vim, `:w FILE` renames the buffer to `FILE` (like `:saveas`).
