# text-tools

Example vox plugin: word count and case changes.

## Commands

| Command / key | Action |
|---|---|
| `:wordcount`, `<leader>c` | show words, lines, and characters |
| `:upper` | change the case of the cursor line (see `case`) |

## Settings

```toml
[plugins.text-tools]
case = "upper"          # upper | lower | title
count_on_save = true    # show the word count after :w
```

## Build

```sh
scripts/build-plugin.sh plugins/text-tools --install ~/.config/vox/plugins
```
