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
    pub(crate) handler: Handler,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}

/// All commands, by name, plus short aliases (`w` → `write`).
#[derive(Debug, Default, Clone)]
pub struct Commands {
    by_name: BTreeMap<String, Command>,
    aliases: BTreeMap<String, String>,
}

impl Commands {
    /// Register (or replace) command `name`.
    pub fn register(
        &mut self,
        name: &str,
        description: &str,
        handler: impl Fn(&mut Editor, Args<'_>) -> CommandResult + 'static,
    ) {
        self.by_name.insert(
            name.to_string(),
            Command {
                name: name.to_string(),
                description: description.to_string(),
                handler: Rc::new(handler),
            },
        );
    }

    /// Make `alias` another name for command `target`.
    pub fn alias(&mut self, alias: &str, target: &str) {
        self.aliases.insert(alias.to_string(), target.to_string());
    }

    /// Look up a command by name or alias.
    pub fn get(&self, name: &str) -> Option<&Command> {
        let name = self.aliases.get(name).map_or(name, String::as_str);
        self.by_name.get(name)
    }

    /// All commands in name order.
    pub fn iter(&self) -> impl Iterator<Item = &Command> {
        self.by_name.values()
    }

    /// Aliases of command `name`, in order.
    pub fn aliases_of<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.aliases
            .iter()
            .filter(move |(_, t)| t.as_str() == name)
            .map(|(a, _)| a.as_str())
    }
}

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
