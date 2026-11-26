# Running commands from a shell

`vox run` runs editor commands without opening the editor. Each argument is one command
line, as typed after `:`.

```sh
vox run "plugin list"
vox run "plugin add https://github.com/me/my-plugin.git@v1.0.0"
vox run "settings set ui.side_width 60" "settings get ui.side_width"
vox run --local-config ~/code/app "plugin remove! old-plugin"
vox run --json "plugin info text-tools"
```

- **Scope:** the same options as the editor. Without options it uses the global setup
  (`~/.config/vox/`); `--local-config [DIR]` uses a project's `.vox/`; `--vanilla` uses
  nothing. Changes are saved to that scope's `config.toml` and `vox.lock`.
- **Order:** commands run one after another. vox waits for background work (installing,
  `plugins sync`) to finish before the next command.
- **Output:** commands that return data (`plugin list`, `settings get`, `scope show`, …)
  print it to stdout as `key: value` text, or with `--json` as one JSON line each.
  Messages and notifications go to stderr as `vox: LEVEL: text`.
- **Errors:** the first failing command stops the run with exit status 1. A command fails
  if it returns an error or shows an error (e.g. a failed install). Bad arguments to
  `vox run` itself give exit status 2.
- **No questions:** nobody can answer a dialog, so commands that open one fail and say what
  to use instead: `:plugin remove! NAME` instead of `:plugin remove NAME`,
  `:settings set KEY VALUE` instead of `:settings`.

## Commands from stdin

`-` reads more command lines from stdin, after the ones given as arguments. Blank lines are
skipped.

```sh
printf '%s\n' \
  "plugin add ./plugins/text-tools" \
  "settings set plugins.text-tools.case lower" \
  "plugin info text-tools" | vox run -
```

## Files

`--file FILE` opens `FILE` first, for commands that work on the buffer:

```sh
vox run --file notes.txt "write notes-copy.txt"
```

## JSON

```sh
$ vox run --vanilla --json "scope show"
{"exists":false,"kind":"vanilla"}
$ vox run --json "plugin list" | jq -r '.[].name'
text-tools
```

Tables are JSON objects with keys in name order; lists are arrays. See
[Commands](commands.md#commands-that-return-data) for the commands that return data.
