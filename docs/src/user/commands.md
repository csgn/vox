# Commands

Commands run from the `:` line or from key mappings (`"<leader>w" = ":w"`). Each command
has a full name and short aliases; both work.

Related commands share one name with a **subcommand**: `:log clear`, `:reload plugins`,
`:scope init`. The root alone either does the main thing (`:log`, `:scope`) or, if it has
none, says which subcommands exist (`:reload` → `Use :reload config|plugins`).

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
| `:help [COMMAND]`, `:h`, `:?COMMAND` | [help](#help) for a command or group; no argument lists all commands |
| `:commands` | list all commands in a modal; `Enter` runs one |
| `:commands list` | all commands as data, with their help (category, usage, params, …) |
| `:commands info COMMAND` | one command's help as data |
| `:commands run COMMAND` | run a command; asks for its arguments first if it needs some |
| `:keymaps` | your key mappings in the side panel |
| `:close` | close the modal or the focused panel |
| `:focus editor\|side\|bottom` | move focus |
| `:log [LEVEL] [SOURCE]` | open the log ([Developer mode](developer-mode.md)) |
| `:log clear`, `:log refresh` | clear / refresh the log |
| `:devmode [on\|off]` | toggle developer mode |
| `:reload config` | re-read the config file |
| `:reload plugins [NAME]` | reload all plugins or one |
| `:checkhealth [NAME]` | plugin health report |
| `:plugin add PATH`, `:plugin add GIT_URL[@REF]` | install into the active scope; updates `vox.lock` ([Plugins](plugins.md)) |
| `:plugins` | plugin manager ([Plugins](plugins.md#plugin-manager)); also `<leader>p` |
| `:plugins help` | plugin manager keys |
| `:plugin remove NAME` | uninstall from the active scope (asks; `:plugin remove! NAME` does not) |
| `:plugin enable NAME`, `:plugin disable NAME` | turn a plugin on/off (saved in `config.toml`) |
| `:plugin list` | plugins of the active scope as data (name, status, path, version) |
| `:plugin info NAME` | one plugin as data (description, authors, settings, health) |
| `:plugin files NAME` | open the plugin's `plugin.toml` |
| `:plugins sync` | install exactly what `vox.lock` lists |
| `:plugin new NAME [DIR]` | create a plugin project ([Writing plugins](../dev/plugins.md)) |
| `:settings [NAME]` | settings dialog ([Settings dialog](settings.md)); `NAME` opens a plugin's settings |
| `:settings vox`, `:settings keys`, `:settings plugin NAME` | open one part directly |
| `:settings key add MODE KEYS TARGET`, `:settings key delete MODE KEYS` | change a mapping in your config |
| `:settings key list` | all key mappings as data (mode, keys, target, owner) |
| `:settings list` | every setting as data (key, value, default, type, description) |
| `:settings get KEY` | a setting's value, e.g. `:settings get ui.side_width`, `:settings get plugins.NAME.KEY` |
| `:settings set KEY VALUE` | change a setting and save it to the scope's `config.toml` (checked against its type and limits) |
| `:settings unset KEY` | remove a setting from `config.toml` (back to its default) |
| `:update check` | look for a newer vox release; asks to install it ([Updating vox](updating.md)) |
| `:update install [VERSION]` | install a release with cargo (default: the newest) |
| `:scope` | show the active scope (global / local / vanilla) and its paths |
| `:scope show` | the active scope and its paths as data |
| `:scope setup` | open the project set-up dialog (local mode) |
| `:scope pick` | checklist of global config and plugins to copy into the project |
| `:scope init empty\|all\|[config] [PLUGIN...]` | create `.vox/` and copy from the global setup |

## Help

`:help COMMAND` opens a dialog with everything about a command: where it sits
(`Commands › Plugins › plugin › add`), what it does, its usage line, parameters (required or
optional, allowed values), what `!` changes, what it returns, and examples. Aliases work
(`:help w`), and so does the short form typed at the `:` prompt: `:?w`, `:?plugin add`.

A group such as `:help plugin` lists its subcommands. Keys in the dialog:

| Key | Action |
|---|---|
| `j` / `k` | select a subcommand (or scroll) |
| `Enter` | open the selected subcommand's help; `Esc` comes back here |
| `r` | run the command; asks for its arguments if it needs some |
| `g` | back to the top |
| `Esc` / `q` | back to the previous dialog, or close |

Plugins add help to their commands too (see [Writing plugins](../dev/plugins.md#command-help)).
`vox run help COMMAND` prints the same help as data.

## Commands that return data

Commands like `:plugin list` and `:settings get KEY` return data. A one-line result is shown
in the message line; a longer one in a modal, as `key: value` lines. The same commands will
be run from a shell with [`vox run`](scripting.md), which prints the data as text or JSON.

Every action has a command that needs no dialog, so it can run without a person at the
keyboard: `:plugin add SOURCE` (not `:plugin add`), `:plugin remove! NAME` (no question),
`:settings set KEY VALUE` (not the form). Without a person, a command that would open a
dialog or ask fails with an error that names the command to use instead.

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
