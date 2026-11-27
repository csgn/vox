//! Command registry: named commands run from the `:` line, from keymaps, and (later) by
//! plugins. Built-in commands are registered the same way as any other.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::Editor;

/// Arguments of one command invocation, e.g. `:w! notes.txt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Args<'a> {
    /// Text after the command name, trimmed. `None` if empty.
    pub arg: Option<&'a str>,
    /// Whether the name ended with `!`.
    pub force: bool,
}

/// What a command returns: `Err(message)` is shown to the user.
pub type CommandResult = Result<(), String>;

/// A command implementation.
pub type Handler = Rc<dyn Fn(&mut Editor, Args<'_>) -> CommandResult>;

/// Names of the built-in command categories. Any other name works too; commands that set
/// none get [`Command::default_category`].
pub mod category {
    /// Open, save, quit.
    pub const FILE: &str = "File";
    /// Panels, modal, notifications, focus.
    pub const INTERFACE: &str = "Interface";
    /// Finding commands and their help.
    pub const HELP: &str = "Help";
    /// Settings and key mappings.
    pub const SETTINGS: &str = "Settings";
    /// Installing and managing plugins.
    pub const PLUGINS: &str = "Plugins";
    /// Scopes and project set-up.
    pub const SCOPE: &str = "Scope";
    /// Log, developer mode, reloading.
    pub const DEVELOPER: &str = "Developer";
    /// Updating vox.
    pub const UPDATE: &str = "Update";
    /// Commands from the user's config (owner `config` or `user`).
    pub const USER: &str = "User";
    /// Built-in commands that set no category.
    pub const OTHER: &str = "Other";
}

/// Where the completions of a parameter come from (used by the `:` prompt's completion).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Complete {
    /// Free text; nothing to complete.
    #[default]
    None,
    /// A file path.
    File,
    /// A directory path.
    Dir,
    /// A command name.
    Command,
    /// A plugin of the active scope.
    Plugin,
    /// A setting key (`ui.side_width`, `plugins.NAME.KEY`).
    SettingKey,
    /// A vox version (`0.17.0`).
    Version,
    /// An open buffer (number or file name).
    Buffer,
    /// One of these words.
    Values(Vec<String>),
}

/// One parameter of a command, for help and completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Shown in usage, e.g. `FILE`.
    pub name: String,
    /// What it is.
    pub description: String,
    /// Must be given.
    pub required: bool,
    /// May be given more than once (`PLUGIN...`).
    pub repeated: bool,
    /// Where completions come from.
    pub complete: Complete,
}

impl Param {
    /// A parameter that must be given.
    pub fn required(name: &str, description: &str) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            required: true,
            repeated: false,
            complete: Complete::None,
        }
    }

    /// A parameter that may be left out.
    pub fn optional(name: &str, description: &str) -> Self {
        Self {
            required: false,
            ..Self::required(name, description)
        }
    }

    /// Allow the parameter more than once.
    pub fn repeated(mut self) -> Self {
        self.repeated = true;
        self
    }

    /// Set where completions come from.
    pub fn complete(mut self, complete: Complete) -> Self {
        self.complete = complete;
        self
    }

    /// Complete from a fixed set of words.
    pub fn values(self, values: &[&str]) -> Self {
        self.complete(Complete::Values(
            values.iter().map(|v| v.to_string()).collect(),
        ))
    }

    /// How the parameter looks in usage: `FILE`, `[FILE]`, `PLUGIN...`, `[PLUGIN...]`.
    pub fn usage(&self) -> String {
        let dots = if self.repeated { "..." } else { "" };
        if self.required {
            format!("{}{dots}", self.name)
        } else {
            format!("[{}{dots}]", self.name)
        }
    }
}

/// An example command line with a short note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    /// The command line, without `:`.
    pub line: String,
    /// What it does.
    pub note: String,
}

/// A registered command.
#[derive(Clone)]
pub struct Command {
    /// Canonical name, e.g. `write`.
    pub name: String,
    /// One-line description for help and docs.
    pub description: String,
    /// Who registered it: `builtin`, `config`, `plugin:NAME`, or `user` (see [`owner`](crate::owner)).
    pub owner: String,
    /// Group it is listed under, e.g. `File` (see [`category`]).
    pub category: String,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// What the command replies with (see [`Editor::reply`]), for data commands.
    pub returns: Option<String>,
    /// What `:name!` does differently; `None` if `!` means nothing.
    pub force: Option<String>,
    /// Example command lines.
    pub examples: Vec<Example>,
    pub(crate) handler: Handler,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("owner", &self.owner)
            .field("category", &self.category)
            .field("params", &self.params)
            .field("returns", &self.returns)
            .field("force", &self.force)
            .field("examples", &self.examples)
            .finish_non_exhaustive()
    }
}

impl Command {
    /// Category used when a command sets none: `Plugin: NAME` for a plugin's commands,
    /// [`category::USER`] for the user's, [`category::OTHER`] otherwise.
    pub fn default_category(owner: &str) -> String {
        if let Some(name) = owner.strip_prefix("plugin:") {
            format!("Plugin: {name}")
        } else if owner == crate::owner::CONFIG || owner == crate::owner::USER {
            category::USER.into()
        } else {
            category::OTHER.into()
        }
    }

    /// Set the category (see [`category`]).
    pub fn category(&mut self, category: &str) -> &mut Self {
        self.category = category.into();
        self
    }

    /// Add a parameter.
    pub fn param(&mut self, param: Param) -> &mut Self {
        self.params.push(param);
        self
    }

    /// Describe what the command replies with.
    pub fn returns(&mut self, returns: &str) -> &mut Self {
        self.returns = Some(returns.into());
        self
    }

    /// Describe what `:name!` does.
    pub fn force(&mut self, force: &str) -> &mut Self {
        self.force = Some(force.into());
        self
    }

    /// Add an example command line (without `:`) and what it does.
    pub fn example(&mut self, line: &str, note: &str) -> &mut Self {
        self.examples.push(Example {
            line: line.into(),
            note: note.into(),
        });
        self
    }

    /// Usage line built from the parameters: `plugin remove[!] NAME`.
    pub fn usage(&self) -> String {
        let mut out = self.name.clone();
        if self.force.is_some() {
            out.push_str("[!]");
        }
        for p in &self.params {
            out.push(' ');
            out.push_str(&p.usage());
        }
        out
    }
}

/// All commands, by name, plus short aliases (`w` → `write`).
///
/// Registering a name that exists hides the old command; removing the newer owner's
/// commands (e.g. unloading a plugin) brings the old one back.
#[derive(Debug, Clone)]
pub struct Commands {
    /// Per name: registrations, oldest first; the last one is active.
    by_name: BTreeMap<String, Vec<Command>>,
    /// Per alias: (target, owner), oldest first; the last one is active.
    aliases: BTreeMap<String, Vec<(String, String)>>,
    /// Owner given to new registrations.
    pub(crate) owner: String,
}

impl Default for Commands {
    fn default() -> Self {
        Self {
            by_name: BTreeMap::new(),
            aliases: BTreeMap::new(),
            owner: crate::owner::USER.into(),
        }
    }
}

impl Commands {
    /// Register command `name`, hiding an existing one with the same name. Returns the
    /// command so metadata can be added: `.category(..).param(..).returns(..)`.
    pub fn register(
        &mut self,
        name: &str,
        description: &str,
        handler: impl Fn(&mut Editor, Args<'_>) -> CommandResult + 'static,
    ) -> &mut Command {
        let cmd = Command {
            name: name.to_string(),
            description: description.to_string(),
            owner: self.owner.clone(),
            category: Command::default_category(&self.owner),
            params: Vec::new(),
            returns: None,
            force: None,
            examples: Vec::new(),
            handler: Rc::new(handler),
        };
        let stack = self.by_name.entry(name.to_string()).or_default();
        stack.retain(|c| c.owner != cmd.owner);
        stack.push(cmd);
        stack.last_mut().expect("just pushed")
    }

    /// The command `name` (no aliases) if `owner` registered it and it is active; used to
    /// add metadata after registering (e.g. from a plugin).
    pub fn get_owned_mut(&mut self, name: &str, owner: &str) -> Option<&mut Command> {
        self.by_name
            .get_mut(name)
            .and_then(|s| s.last_mut())
            .filter(|c| c.owner == owner)
    }

    /// Make `alias` another name for command `target`.
    pub fn alias(&mut self, alias: &str, target: &str) {
        let owner = self.owner.clone();
        let stack = self.aliases.entry(alias.to_string()).or_default();
        stack.retain(|(_, o)| *o != owner);
        stack.push((target.to_string(), owner));
    }

    /// Look up a command by name or alias.
    pub fn get(&self, name: &str) -> Option<&Command> {
        let name = self
            .aliases
            .get(name)
            .and_then(|s| s.last())
            .map_or(name, |(t, _)| t.as_str());
        self.by_name.get(name).and_then(|s| s.last())
    }

    /// All active commands in name order.
    pub fn iter(&self) -> impl Iterator<Item = &Command> {
        self.by_name.values().filter_map(|s| s.last())
    }

    /// Aliases of command `name`, in order.
    pub fn aliases_of<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.aliases
            .iter()
            .filter(move |(_, s)| s.last().is_some_and(|(t, _)| t == name))
            .map(|(a, _)| a.as_str())
    }

    /// Subcommands of `root` (commands named `"ROOT SUB"`, e.g. `reload config`), by name.
    pub fn subcommands_of<'a>(&'a self, root: &'a str) -> impl Iterator<Item = &'a Command> + 'a {
        self.iter().filter(move |c| {
            c.name
                .strip_prefix(root)
                .is_some_and(|rest| rest.starts_with(' '))
        })
    }

    /// Whether `root` has subcommands (`plugin` has `plugin add`), even if `root` itself is
    /// not a command.
    pub fn has_subcommands(&self, root: &str) -> bool {
        self.subcommands_of(root).next().is_some()
    }

    /// Remove every command and alias registered by `owner`.
    pub fn remove_owned(&mut self, owner: &str) {
        for stack in self.by_name.values_mut() {
            stack.retain(|c| c.owner != owner);
        }
        self.by_name.retain(|_, s| !s.is_empty());
        for stack in self.aliases.values_mut() {
            stack.retain(|(_, o)| o != owner);
        }
        self.aliases.retain(|_, s| !s.is_empty());
    }
}

/// Command names may contain one space for a subcommand: registering `"reload config"`
/// makes `:reload config` work, and different owners can add subcommands to the same root.
///
/// Split a command line into name, force flag, and argument: `"w! a.txt"` →
/// `("w", Args { arg: Some("a.txt"), force: true })`.
pub fn parse_line(line: &str) -> (&str, Args<'_>) {
    let line = line.trim();
    let (name, arg) = match line.split_once(char::is_whitespace) {
        Some((n, a)) => (n, Some(a.trim()).filter(|a| !a.is_empty())),
        None => (line, None),
    };
    match name.strip_suffix('!') {
        Some(n) => (n, Args { arg, force: true }),
        None => (name, Args { arg, force: false }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_line_cases() {
        assert_eq!(
            parse_line("w"),
            (
                "w",
                Args {
                    arg: None,
                    force: false
                }
            )
        );
        assert_eq!(
            parse_line(" e!  notes.txt "),
            (
                "e",
                Args {
                    arg: Some("notes.txt"),
                    force: true
                }
            )
        );
        assert_eq!(parse_line("").0, "");
    }

    #[test]
    fn newer_owner_hides_and_removal_restores() {
        let mut c = Commands {
            owner: "builtin".into(),
            ..Commands::default()
        };
        c.register("write", "builtin write", |_, _| Ok(()));
        c.alias("w", "write");
        c.owner = "plugin:x".into();
        c.register("write", "plugin write", |_, _| Ok(()));
        c.register("extra", "", |_, _| Ok(()));
        c.alias("w", "extra");
        assert_eq!(c.get("write").unwrap().description, "plugin write");
        assert_eq!(c.get("w").unwrap().name, "extra");
        c.remove_owned("plugin:x");
        assert_eq!(c.get("write").unwrap().description, "builtin write");
        assert_eq!(c.get("w").unwrap().name, "write");
        assert!(c.get("extra").is_none());
        assert_eq!(c.iter().count(), 1);
    }

    #[test]
    fn metadata_and_usage() {
        let mut c = Commands::default();
        c.register("plugin remove", "Uninstall", |_, _| Ok(()))
            .category(category::PLUGINS)
            .param(Param::required("NAME", "plugin").complete(Complete::Plugin))
            .param(Param::optional("MORE", "more plugins").repeated())
            .force("do not ask")
            .example("plugin remove! x", "remove x");
        let cmd = c.get("plugin remove").unwrap();
        assert_eq!(cmd.usage(), "plugin remove[!] NAME [MORE...]");
        assert_eq!(cmd.category, "Plugins");
        assert_eq!(cmd.examples[0].line, "plugin remove! x");
        assert!(c.has_subcommands("plugin"));
        assert!(!c.has_subcommands("plugin remove"));
    }

    #[test]
    fn default_category_by_owner() {
        assert_eq!(Command::default_category("plugin:tt"), "Plugin: tt");
        assert_eq!(Command::default_category("config"), "User");
        assert_eq!(Command::default_category("user"), "User");
        assert_eq!(Command::default_category("builtin"), "Other");
        let mut c = Commands {
            owner: "plugin:tt".into(),
            ..Commands::default()
        };
        c.register("wc", "", |_, _| Ok(()));
        assert!(c.get_owned_mut("wc", "plugin:other").is_none());
        c.get_owned_mut("wc", "plugin:tt").unwrap().category("Text");
        assert_eq!(c.get("wc").unwrap().category, "Text");
    }

    #[test]
    fn register_alias_lookup() {
        let mut c = Commands::default();
        c.register("write", "Save", |_, _| Ok(()));
        c.alias("w", "write");
        assert_eq!(c.get("w").unwrap().name, "write");
        assert_eq!(c.get("write").unwrap().description, "Save");
        assert!(c.get("nope").is_none());
        assert_eq!(c.aliases_of("write").collect::<Vec<_>>(), ["w"]);
    }
}
