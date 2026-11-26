# Commands

Commands run from the `:` line or from key mappings (`"<leader>w" = ":w"`). Each command
has a full name and short aliases; both work.

| Command | Action |
|---|---|
| `:write`, `:w` | write to the current file |
| `:w FILE` | write to `FILE` and make it the current file |
| `:q`, `:quit`, `:qa` | quit; refuses with `E37` if there are unsaved changes |
| `:q!` | quit, discarding changes |
| `:wq [FILE]` | write, then quit |
| `:xit`, `:x [FILE]` | write if modified, then quit |
| `:edit`, `:e FILE` | open `FILE`; refuses with `E37` if there are unsaved changes |
| `:e` | reload the current file |
| `:e! [FILE]` | open/reload, discarding changes |
| `:notify [info\|warn\|error] TEXT` | show a notification |
| `:messages`, `:mes` | message history in the bottom panel |
| `:commands` | list all commands in a modal; `Enter` runs one |
| `:keymaps` | your key mappings in the side panel |
| `:close` | close the modal or the focused panel |
| `:focus editor\|side\|bottom` | move focus |

## Messages
| Message | Meaning |
|---|---|
| `"f" NL, NB written` | write succeeded |
| `E32: No file name` | `:w` on an unnamed buffer |
| `E37: No write since last change` | unsaved changes; add `!` to override |
| `E212: Can't open file for writing` | write failed |
| `E484: Can't open file` | `:e` failed |
| `E492: Not an editor command` | unknown command |
| `E35: No previous regular expression` | `n`/`N` before any search |
| `E486: Pattern not found` | search found nothing |
| `E353: Nothing in register` | paste with empty register |
| `E354: Invalid register name` | register other than `"+`, `"*` |

Note: unlike Vim, `:w FILE` renames the buffer to `FILE` (like `:saveas`).
