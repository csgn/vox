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

/// A registered command.
#[derive(Clone)]
pub struct Command {
    /// Canonical name, e.g. `write`.
    pub name: String,
    /// One-line description for help and docs.
    pub description: String,
    /// Who registered it: `builtin`, `config`, `plugin:NAME`, or `user` (see [`owner`](crate::owner)).
    pub owner: String,
    pub(crate) handler: Handler,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("owner", &self.owner)
            .finish_non_exhaustive()
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
    /// Register command `name`, hiding an existing one with the same name.
    pub fn register(
        &mut self,
        name: &str,
        description: &str,
        handler: impl Fn(&mut Editor, Args<'_>) -> CommandResult + 'static,
    ) {
        let cmd = Command {
            name: name.to_string(),
            description: description.to_string(),
            owner: self.owner.clone(),
            handler: Rc::new(handler),
        };
        let stack = self.by_name.entry(name.to_string()).or_default();
        stack.retain(|c| c.owner != cmd.owner);
        stack.push(cmd);
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
