//! Completion of `:` command lines: command names, subcommands, and arguments from each
//! parameter's [`Complete`] source. Pure: reads editor state (and the file system for
//! paths), changes nothing. Used by the `:` prompt and `:commands complete`.

use crate::Editor;
use crate::command::{Command, Commands, Complete};

/// One completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Text that replaces the word being completed.
    pub text: String,
    /// Short description shown next to it.
    pub detail: String,
}

/// What can be typed at the end of a command line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completion {
    /// Byte offset in the line where the replaced word starts.
    pub start: usize,
    /// Matches: those starting with the typed text; if there are none, those containing its
    /// letters in order (fuzzy).
    pub items: Vec<Candidate>,
    /// Usage of the command being typed, e.g. `plugin add SOURCE`.
    pub usage: Option<String>,
    /// The parameter being typed (name as in usage), to highlight in the usage.
    pub param: Option<String>,
}

impl Completion {
    /// The line with candidate `i` put in place of the word being completed.
    pub fn apply(&self, line: &str, i: usize) -> String {
        let text = self.items.get(i).map_or("", |c| c.text.as_str());
        format!("{}{text}", &line[..self.start.min(line.len())])
    }

    /// Longest text all prefix matches share, if longer than what is typed.
    pub fn common_prefix(&self, line: &str) -> Option<String> {
        let typed = &line[self.start.min(line.len())..];
        let mut prefix: Option<&str> = None;
        for c in self.items.iter().filter(|c| c.text.starts_with(typed)) {
            prefix = Some(match prefix {
                None => &c.text,
                Some(p) => {
                    let n = p
                        .char_indices()
                        .zip(c.text.chars())
                        .take_while(|((_, a), b)| a == b)
                        .last()
                        .map_or(0, |((i, a), _)| i + a.len_utf8());
                    &p[..n]
                }
            });
        }
        prefix
            .filter(|p| p.len() > typed.len())
            .map(|p| format!("{}{p}", &line[..self.start]))
    }
}

/// How well `word` matches `typed`: `Some(0)` prefix, `Some(1)` letters in order, `None`.
/// Case-insensitive.
fn score(word: &str, typed: &str) -> Option<u8> {
    let (w, t) = (word.to_lowercase(), typed.to_lowercase());
    if w.starts_with(&t) {
        return Some(0);
    }
    let mut chars = w.chars();
    t.chars().all(|c| chars.any(|x| x == c)).then_some(1)
}

/// Keep the candidates starting with `typed`; if none do, those matching it fuzzily. Order
/// is kept; duplicates are dropped.
fn filter(items: Vec<Candidate>, typed: &str) -> Vec<Candidate> {
    let scored: Vec<(u8, Candidate)> = items
        .into_iter()
        .filter_map(|c| score(&c.text, typed).map(|s| (s, c)))
        .collect();
    let best = scored.iter().map(|(s, _)| *s).min().unwrap_or(0);
    let mut seen = std::collections::BTreeSet::new();
    scored
        .into_iter()
        .filter(|(s, _)| *s == best)
        .map(|(_, c)| c)
        .filter(|c| seen.insert(c.text.clone()))
        .collect()
}

/// Root command words and aliases, with a description.
fn roots(commands: &Commands) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    for c in commands.iter() {
        let root = c.name.split(' ').next().unwrap_or_default();
        if out.iter().any(|o| o.text == root) {
            continue;
        }
        let detail = match commands.get(root) {
            Some(cmd) => cmd.description.clone(),
            None => format!("{} subcommands", commands.subcommands_of(root).count()),
        };
        out.push(Candidate {
            text: root.to_string(),
            detail,
        });
    }
    for c in commands.iter() {
        for a in commands.aliases_of(&c.name) {
            out.push(Candidate {
                text: a.to_string(),
                detail: format!("→ {}", c.name),
            });
        }
    }
    out
}

/// Every command name (with subcommands) and alias: for parameters naming a command.
fn command_names(commands: &Commands) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = commands
        .iter()
        .map(|c| Candidate {
            text: c.name.clone(),
            detail: c.description.clone(),
        })
        .collect();
    for c in commands.iter() {
        for a in commands.aliases_of(&c.name) {
            out.push(Candidate {
                text: a.to_string(),
                detail: format!("→ {}", c.name),
            });
        }
    }
    out
}

/// Files (or only folders) for the typed path: entries of its folder whose names start
/// with the typed name; folders end with `/`. Hidden entries only when the name starts
/// with `.`.
fn paths(typed: &str, dirs_only: bool) -> Vec<Candidate> {
    let (dir, name) = match typed.rfind('/') {
        Some(i) => (&typed[..=i], &typed[i + 1..]),
        None => ("", typed),
    };
    let read = if dir.is_empty() { "." } else { dir };
    let read = match read.strip_prefix("~/") {
        Some(rest) => {
            std::env::var_os("HOME").map_or(read.into(), |h| std::path::Path::new(&h).join(rest))
        }
        None => read.into(),
    };
    let Ok(entries) = std::fs::read_dir(read) else {
        return Vec::new();
    };
    let mut out: Vec<Candidate> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            if (n.starts_with('.') && !name.starts_with('.')) || !n.starts_with(name) {
                return None;
            }
            let is_dir = e.path().is_dir();
            if dirs_only && !is_dir {
                return None;
            }
            let slash = if is_dir { "/" } else { "" };
            Some(Candidate {
                text: format!("{dir}{n}{slash}"),
                detail: String::new(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.text.cmp(&b.text));
    out
}

/// Candidates of a parameter's source, before filtering.
fn source(ed: &Editor, complete: &Complete, typed: &str) -> Vec<Candidate> {
    let plain = |s: &str, d: &str| Candidate {
        text: s.to_string(),
        detail: d.to_string(),
    };
    match complete {
        Complete::None | Complete::Version => Vec::new(),
        Complete::File => paths(typed, false),
        Complete::Dir => paths(typed, true),
        Complete::Command => command_names(ed.commands()),
        Complete::Plugin => ed
            .plugin_infos()
            .iter()
            .map(|i| plain(&i.name, i.status.name()))
            .collect(),
        Complete::SettingKey => crate::settings::setting_keys(ed)
            .into_iter()
            .map(|(k, d)| plain(&k, &d))
            .collect(),
        Complete::Values(v) => v.iter().map(|s| plain(s, "")).collect(),
    }
}

/// Byte ranges of the whitespace-separated words of `line`.
fn words(line: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in line.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(s)) => {
                out.push((s, &line[s..i]));
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, &line[s..]));
    }
    out
}

impl Editor {
    /// Completions for the end of command line `line` (the text after `:`).
    pub fn complete(&self, line: &str) -> Completion {
        // `:?TOPIC` is `:help TOPIC`.
        if let Some(rest) = line.strip_prefix('?') {
            let mut c = self.complete(&format!("help {rest}"));
            c.start = c.start + 1 - "help ".len();
            return c;
        }
        let commands = self.commands();
        let ws = words(line);
        let ends_in_space = line.ends_with(char::is_whitespace) || line.is_empty();
        // The word being typed and the words before it.
        let (start, typed, before): (usize, &str, &[(usize, &str)]) = match ws.split_last() {
            Some((&(s, w), rest)) if !ends_in_space => (s, w, rest),
            _ => (line.len(), "", ws.as_slice()),
        };
        let Some(&(_, first)) = before.first() else {
            if typed.is_empty() && line.trim().is_empty() {
                return Completion {
                    start,
                    items: roots(commands),
                    ..Completion::default()
                };
            }
            return Completion {
                start,
                items: filter(roots(commands), typed),
                ..Completion::default()
            };
        };
        let root_word = first.trim_end_matches('!');
        // `e` → `edit`; a root that is only a group (`plugin`) stays as typed.
        let root = commands
            .get(root_word)
            .map_or(root_word.to_string(), |c| c.name.clone());
        let mut items = Vec::new();
        // The command: `ROOT SUB` if the second word names a subcommand, else `ROOT`.
        let (cmd, args): (Option<&Command>, &[(usize, &str)]) = match before.get(1) {
            Some(&(_, sub))
                if commands
                    .get(&format!("{root} {}", sub.trim_end_matches('!')))
                    .is_some() =>
            {
                (
                    commands.get(&format!("{root} {}", sub.trim_end_matches('!'))),
                    &before[2..],
                )
            }
            _ => (commands.get(&root), &before[1..]),
        };
        if before.len() == 1 && commands.has_subcommands(&root) {
            items.extend(commands.subcommands_of(&root).map(|c| Candidate {
                text: c.name[root.len() + 1..].to_string(),
                detail: c.description.clone(),
            }));
        }
        let Some(cmd) = cmd else {
            return Completion {
                start,
                items: filter(items, typed),
                ..Completion::default()
            };
        };
        // A parameter naming a command takes the rest of the line (`help plugin add`).
        let rest_param = cmd
            .params
            .iter()
            .position(|p| p.complete == Complete::Command)
            .filter(|&k| k <= args.len());
        let index = rest_param.unwrap_or(args.len());
        let param = cmd
            .params
            .get(index)
            .or_else(|| cmd.params.last().filter(|p| p.repeated));
        let mut completion = Completion {
            start,
            usage: Some(cmd.usage()),
            param: param.map(|p| p.name.clone()),
            ..Completion::default()
        };
        if let Some(p) = param {
            if p.complete == Complete::Command {
                let from = args.get(index).map_or(start, |&(s, _)| s);
                let rest = &line[from..];
                let names = command_names(commands)
                    .into_iter()
                    .filter(|c| c.text.starts_with(rest.trim_start()))
                    .collect::<Vec<_>>();
                let names = if names.is_empty() && !rest.contains(' ') {
                    filter(command_names(commands), rest)
                } else {
                    names
                };
                completion.start = from;
                completion.items = names;
                return completion;
            }
            items.extend(source(self, &p.complete, typed));
        }
        completion.items = match &param.map(|p| &p.complete) {
            // Paths are already narrowed to the typed folder and name.
            Some(Complete::File | Complete::Dir) => items,
            _ => filter(items, typed),
        };
        completion
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(c: &Completion) -> Vec<&str> {
        c.items.iter().map(|i| i.text.as_str()).collect()
    }

    #[test]
    fn scoring() {
        assert_eq!(score("write", "wr"), Some(0));
        assert_eq!(score("settings", "stg"), Some(1));
        assert_eq!(score("write", "x"), None);
        assert_eq!(score("Write", "w"), Some(0));
    }

    #[test]
    fn words_and_apply() {
        assert_eq!(words(" a  bc "), [(1, "a"), (4, "bc")]);
        let c = Completion {
            start: 7,
            items: vec![Candidate {
                text: "add".into(),
                detail: String::new(),
            }],
            ..Completion::default()
        };
        assert_eq!(c.apply("plugin a", 0), "plugin add");
        assert_eq!(c.common_prefix("plugin a"), Some("plugin add".into()));
        assert_eq!(c.common_prefix("plugin add"), None);
    }

    #[test]
    fn names_subcommands_and_values() {
        let ed = Editor::new(vox_core::Document::from_text(""));
        let c = ed.complete("wri");
        assert_eq!(texts(&c)[0], "write");
        assert_eq!(c.start, 0);
        // Fuzzy after prefix matches.
        let c = ed.complete("stg");
        assert!(texts(&c).contains(&"settings"));
        let c = ed.complete("plugin a");
        assert_eq!(texts(&c), ["add"]);
        assert_eq!(c.start, 7);
        let c = ed.complete("plugin ");
        assert!(texts(&c).contains(&"remove"));
        assert_eq!(c.usage, None, "plugin alone is not a command");
        let c = ed.complete("notify ");
        assert_eq!(texts(&c), ["info", "warn", "error"]);
        assert_eq!(c.usage.as_deref(), Some("notify [LEVEL] TEXT"));
        assert_eq!(c.param.as_deref(), Some("LEVEL"));
        let c = ed.complete("notify warn ");
        assert!(c.items.is_empty());
        assert_eq!(c.param.as_deref(), Some("TEXT"));
        // Alias as the root.
        let c = ed.complete("e! ");
        assert_eq!(c.usage.as_deref(), Some("edit[!] [FILE]"));
    }

    #[test]
    fn command_parameters_take_the_rest_of_the_line() {
        let ed = Editor::new(vox_core::Document::from_text(""));
        let c = ed.complete("help plugin a");
        assert_eq!(texts(&c), ["plugin add"]);
        assert_eq!(c.start, 5);
        assert_eq!(c.apply("help plugin a", 0), "help plugin add");
        let c = ed.complete("?plugin a");
        assert_eq!(c.apply("?plugin a", 0), "?plugin add");
        let c = ed.complete("help wr");
        assert_eq!(texts(&c)[0], "write");
    }

    #[test]
    fn setting_keys_and_paths() {
        let ed = Editor::new(vox_core::Document::from_text(""));
        let c = ed.complete("settings get ui.side_w");
        assert_eq!(texts(&c), ["ui.side_width"]);
        let dir = std::env::temp_dir().join(format!("vox-complete-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("notes.txt"), "").unwrap();
        std::fs::write(dir.join(".hidden"), "").unwrap();
        let d = dir.display();
        let c = ed.complete(&format!("e {d}/"));
        assert_eq!(texts(&c), [format!("{d}/notes.txt"), format!("{d}/sub/")]);
        let c = ed.complete(&format!("e {d}/n"));
        assert_eq!(texts(&c), [format!("{d}/notes.txt")]);
        let c = ed.complete(&format!("e {d}/."));
        assert_eq!(texts(&c), [format!("{d}/.hidden")]);
        let c = ed.complete(&format!("plugin add {d}/"));
        assert_eq!(texts(&c), [format!("{d}/sub/")]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
