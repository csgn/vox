# Project setups (scopes)

Your global config and plugins are the default. Some of them do not fit every project, so a
project can have **its own** setup instead. vox always uses exactly one **scope**:

| Scope | Start with | Config | Plugins |
|---|---|---|---|
| **global** | `vox [FILE]` | `~/.config/vox/config.toml` | `~/.config/vox/plugins/` |
| **local** | `vox --local-config [DIR] [FILE]` | `<project>/.vox/config.toml` | `<project>/.vox/plugins/` |
| **vanilla** | `vox --vanilla [FILE]` | none | none |

- In **local** mode the global config and plugins are **not loaded at all**. The status line
  shows `LOCAL`.
- In **vanilla** mode nothing is loaded: plain vox with default keys. The status line shows
  `VANILLA`.
- `:scope` shows the active scope and its paths.

## Which project?

`--local-config DIR` uses the project folder `DIR`. Without `DIR`, vox looks for the nearest
folder at or above the current folder that contains `.vox/` (like git finds a repository). If
there is none, the current folder becomes the project.

## Setting up a project

The first time you start a project in local mode (no `.vox/` yet), vox asks what to put in it:

| Choice | Result |
|---|---|
| Start empty | `.vox/` with no config and no plugins |
| Copy global config | your `config.toml` is copied |
| Choose what to copy… | a checklist of your config and each global plugin (`Space` toggles, `a` all, `Enter` copies, `Esc` back to the choices) |
| Copy everything | config and all global plugins |
| Not now | nothing is created; run `:scope setup` any time |

Copied files belong to the project from then on: changing them does not change your global
setup, and vox never overwrites files that already exist in `.vox/`. After copying, the
project's config and plugins are loaded right away.

The same actions are commands: `:scope setup`, `:scope pick`, and
`:scope init empty|all|[config] [PLUGIN...]` (e.g. `:scope init config text_tools`).

## Where the global setup lives

`$VOX_CONFIG_DIR` if set, else `$XDG_CONFIG_HOME/vox`, else `~/.config/vox`.

## Lockfile

Each scope has a `vox.lock` listing its plugins with exact versions and sources (see
[Plugins](plugins.md#the-lockfile)). The project set-up also copies the global lock entries of
the plugins it copies, and writes `.vox/.gitignore` so only `config.toml` and `vox.lock` are
committed. After cloning a project: `vox --local-config`, then `:plugins sync`.
