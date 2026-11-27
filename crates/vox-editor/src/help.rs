//! Command help: `:help CMD` (also `:?CMD`) opens a dialog with a command's description,
//! usage, parameters, result, and examples, and lists its subcommands; `:commands info CMD`
//! returns the same as data; `:commands run CMD` runs a command, asking for its arguments.
//! `:commands` opens the command browser: categories → commands → subcommands.

use std::cell::Cell;

use vox_view::CharWidth;

use crate::Editor;
use crate::command::category::HELP;
use crate::command::{Args, Command, CommandResult, Commands, Complete, Param};
use crate::config::Value;
use crate::keys::{Key, KeyCode};
use crate::surface::{Color, Rect, Style, Surface};
use crate::ui::{Input, ManagerPlace, Outcome, PanelSlot, TreeNode, TreeView, View, wrap};

/// Register `:help`, `:commands info`, and `:commands run`.
pub(crate) fn register(c: &mut Commands) {
    c.register(
        "help",
        "Show a command's help: description, usage, parameters, result, examples, subcommands.",
        help,
    )
    .category(HELP)
    .param(
        Param::optional(
            "COMMAND",
            "A command, alias, or command group (`plugin`); none lists all commands.",
        )
        .complete(Complete::Command),
    )
    .returns("Without the editor UI (`vox run help CMD`): the same table as `:commands info`.")
    .example("help plugin add", "help for :plugin add")
    .example("?w", "short form, typed at the `:` prompt");
    c.alias("h", "help");
    c.register(
        "commands info",
        "Return a command's help as data.",
        info,
    )
    .category(HELP)
    .param(Param::required("COMMAND", "A command, alias, or command group.").complete(Complete::Command))
    .returns(
        "A table: name, description, owner, aliases, category, usage, params (name, description, \
         required, repeated, complete, values), returns, force, examples (line, note), subcommands.",
    );
    c.register(
        "commands run",
        "Run a command; if it needs arguments, ask for them first.",
        run,
    )
    .category(HELP)
    .param(Param::required("COMMAND", "The command to run.").complete(Complete::Command));
}

/// What a help topic names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Topic {
    /// A command, by canonical name.
    Command(String),
    /// A root that only has subcommands, e.g. `plugin`.
    Group(String),
}

/// Find what `topic` names: a command or alias (`w`, `plugin add`, `:e!`), or a group of
/// subcommands (`plugin`). Leading `:` and trailing `!` are ignored.
pub fn resolve(commands: &Commands, topic: &str) -> Option<Topic> {
    let words: Vec<&str> = topic
        .trim()
        .trim_start_matches(':')
        .split_whitespace()
        .map(|w| w.trim_end_matches('!'))
        .collect();
    let name = words.join(" ");
    if name.is_empty() {
        return None;
    }
    if let Some(cmd) = commands.get(&name) {
        return Some(Topic::Command(cmd.name.clone()));
    }
    // An alias as the root of a subcommand: `:h` is not a group, but `x sub` could be.
    if let Some((root, rest)) = name.split_once(' ')
        && let Some(cmd) = commands.get(root)
        && let Some(sub) = commands.get(&format!("{} {rest}", cmd.name))
    {
        return Some(Topic::Command(sub.name.clone()));
    }
    commands
        .has_subcommands(&name)
        .then_some(Topic::Group(name))
}

/// The error for a topic that names nothing.
fn no_help(topic: &str) -> String {
    format!("E149: Sorry, no help for {}", topic.trim())
}

/// `:help [COMMAND]`.
fn help(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let Some(topic) = a.arg else {
        ed.run_command("commands");
        return Ok(());
    };
    let found = resolve(ed.commands(), topic).ok_or_else(|| no_help(topic))?;
    if !ed.interactive() {
        ed.reply(topic_value(ed.commands(), &found));
        return Ok(());
    }
    let view = HelpView::new(ed.commands(), &found);
    ed.open_modal(Box::new(view));
    Ok(())
}

/// `:commands info COMMAND`.
fn info(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let topic = a.arg.ok_or("E471: Use :commands info COMMAND")?;
    let found = resolve(ed.commands(), topic).ok_or_else(|| no_help(topic))?;
    ed.reply(topic_value(ed.commands(), &found));
    Ok(())
}

/// `:commands run COMMAND`: run it now, or ask for its arguments first.
fn run(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let topic = a.arg.ok_or("E471: Use :commands run COMMAND")?;
    let Some(Topic::Command(name)) = resolve(ed.commands(), topic) else {
        return Err(format!("E492: Not an editor command: {}", topic.trim()));
    };
    let cmd = ed.commands().get(&name).expect("resolved");
    if !cmd.params.iter().any(|p| p.required) {
        ed.run_command(&name);
        return Ok(());
    }
    let usage = cmd.usage();
    if !ed.interactive() {
        return Err(format!("E471: :{name} needs arguments: :{usage}"));
    }
    let input = Input::new(
        format!("Run :{usage}"),
        format!(":{name} "),
        format!("{name} {{}}"),
    );
    ed.open_modal(Box::new(input));
    Ok(())
}

/// `:commands`: the command browser, as a modal or in the side panel (`[ui] commands`).
pub(crate) fn open_browser(ed: &mut Editor) {
    let nodes = command_tree(ed.commands());
    let view = TreeView::new("Commands", "Commands", nodes);
    match ed.ui_config().commands {
        ManagerPlace::Modal => ed.open_modal(Box::new(view)),
        ManagerPlace::Side => ed.open_panel(PanelSlot::Side, Box::new(view.in_panel())),
    }
}

/// Sort key of a category: built-in categories first, then plugins, then the user's.
fn category_rank(name: &str) -> (u8, &str) {
    let rank = if name.starts_with("Plugin: ") {
        1
    } else if name == crate::command::category::USER {
        2
    } else if name == crate::command::category::OTHER {
        3
    } else {
        0
    };
    (rank, name)
}

/// A command as a browser entry labelled `label`: runs it on `Enter`, help on `?`.
fn command_node(commands: &Commands, c: &Command, label: &str) -> TreeNode {
    let aliases: Vec<&str> = commands.aliases_of(&c.name).collect();
    let label = if aliases.is_empty() {
        label.to_string()
    } else {
        format!("{label} ({})", aliases.join(", "))
    };
    TreeNode {
        label,
        detail: c.description.clone(),
        info: vec![c.description.clone(), format!("Usage  :{}", c.usage())],
        run: Some(format!("commands run {}", c.name)),
        help: Some(format!("help {}", c.name)),
        children: Vec::new(),
    }
}

/// The command browser's tree: categories → root commands → subcommands. A root that is a
/// command and has subcommands lists itself first. Within a category only the commands of
/// that category are shown, so one root can appear in several categories.
pub fn command_tree(commands: &Commands) -> Vec<TreeNode> {
    use std::collections::BTreeMap;
    // category → root → commands (in name order, the root itself first).
    let mut groups: BTreeMap<(u8, String), BTreeMap<String, Vec<&Command>>> = BTreeMap::new();
    for c in commands.iter() {
        let (rank, _) = category_rank(&c.category);
        let root = c.name.split(' ').next().unwrap_or_default().to_string();
        groups
            .entry((rank, c.category.clone()))
            .or_default()
            .entry(root)
            .or_default()
            .push(c);
    }
    groups
        .into_iter()
        .map(|((_, category), roots)| {
            let count: usize = roots.values().map(Vec::len).sum();
            let children = roots
                .into_iter()
                .map(|(root, cmds)| {
                    if let [only] = cmds.as_slice()
                        && only.name == root
                    {
                        return command_node(commands, only, &root);
                    }
                    let own = cmds.iter().find(|c| c.name == root);
                    let detail = own.map_or_else(
                        || format!("{} commands", cmds.len()),
                        |c| c.description.clone(),
                    );
                    let info = vec![match own {
                        Some(c) => c.description.clone(),
                        None => format!("Commands under :{root}."),
                    }];
                    let children = cmds
                        .iter()
                        .map(|c| {
                            let label = c.name.strip_prefix(&format!("{root} ")).unwrap_or(&root);
                            command_node(commands, c, label)
                        })
                        .collect();
                    TreeNode {
                        label: root.clone(),
                        detail,
                        info,
                        run: None,
                        help: Some(format!("help {root}")),
                        children,
                    }
                })
                .collect();
            TreeNode {
                label: category.clone(),
                detail: format!("{count} commands"),
                info: vec![format!("{category}: {count} commands")],
                run: None,
                help: None,
                children,
            }
        })
        .collect()
}

/// Category of a group: that of its first subcommand.
fn group_category(commands: &Commands, root: &str) -> String {
    commands
        .subcommands_of(root)
        .next()
        .map(|c| c.category.clone())
        .unwrap_or_default()
}

/// Subcommands of `root` as (full name, description).
fn subcommands(commands: &Commands, root: &str) -> Vec<(String, String)> {
    commands
        .subcommands_of(root)
        .map(|c| (c.name.clone(), c.description.clone()))
        .collect()
}

/// Name of a [`Complete`] source in data: `file`, `setting-key`, `values`, ….
fn complete_name(c: &Complete) -> &'static str {
    match c {
        Complete::None => "none",
        Complete::File => "file",
        Complete::Dir => "dir",
        Complete::Command => "command",
        Complete::Plugin => "plugin",
        Complete::SettingKey => "setting-key",
        Complete::Version => "version",
        Complete::Buffer => "buffer",
        Complete::Values(_) => "values",
    }
}

/// A parameter as data.
fn param_value(p: &Param) -> Value {
    let mut pairs = vec![
        ("name", Value::from(p.name.as_str())),
        ("description", p.description.as_str().into()),
        ("required", p.required.into()),
        ("repeated", p.repeated.into()),
        ("complete", complete_name(&p.complete).into()),
    ];
    if let Complete::Values(v) = &p.complete {
        pairs.push((
            "values",
            Value::Array(v.iter().map(|s| s.as_str().into()).collect()),
        ));
    }
    Value::table(pairs)
}

/// A command with all its metadata as data (used by `:commands list` and `:commands info`).
pub(crate) fn command_value(commands: &Commands, c: &Command) -> Value {
    let aliases = commands.aliases_of(&c.name).map(Value::from).collect();
    let examples = c
        .examples
        .iter()
        .map(|e| {
            Value::table([
                ("line", Value::from(e.line.as_str())),
                ("note", e.note.as_str().into()),
            ])
        })
        .collect();
    let subs = commands
        .subcommands_of(&c.name)
        .map(|s| Value::from(s.name.as_str()))
        .collect();
    let mut pairs = vec![
        ("name", Value::from(c.name.as_str())),
        ("description", c.description.as_str().into()),
        ("owner", c.owner.as_str().into()),
        ("aliases", Value::Array(aliases)),
        ("category", c.category.as_str().into()),
        ("usage", c.usage().into()),
        (
            "params",
            Value::Array(c.params.iter().map(param_value).collect()),
        ),
        ("examples", Value::Array(examples)),
        ("subcommands", Value::Array(subs)),
    ];
    if let Some(r) = &c.returns {
        pairs.push(("returns", r.as_str().into()));
    }
    if let Some(f) = &c.force {
        pairs.push(("force", f.as_str().into()));
    }
    Value::table(pairs)
}

/// A topic as data.
fn topic_value(commands: &Commands, topic: &Topic) -> Value {
    match topic {
        Topic::Command(name) => command_value(commands, commands.get(name).expect("resolved")),
        Topic::Group(root) => {
            let subs = commands
                .subcommands_of(root)
                .map(|s| Value::from(s.name.as_str()))
                .collect();
            Value::table([
                ("name", Value::from(root.as_str())),
                ("description", format!("Commands under :{root}.").into()),
                ("category", group_category(commands, root).into()),
                ("subcommands", Value::Array(subs)),
            ])
        }
    }
}

/// Width of the label column (`Usage`, `Category`, …).
const LABEL: usize = 10;

/// One line of the help text: style, text, and the indent of wrapped continuation rows.
type Line = (Style, String, usize);

/// The help dialog of one command or command group. Keys: `j`/`k` move through the
/// subcommands (or scroll), `Enter` opens the selected subcommand's help, `r` runs the
/// command, `Esc`/`q` go back.
#[derive(Debug, Clone)]
pub struct HelpView {
    /// `:help plugin add`.
    title: String,
    /// `Commands › Plugins › plugin › add`.
    path: String,
    /// Text above the subcommands.
    body: Vec<Line>,
    /// Subcommands: (full name, description).
    subs: Vec<(String, String)>,
    /// Selected subcommand; `None` until `j` is pressed.
    selected: Option<usize>,
    /// First visible row (when nothing is selected).
    top: usize,
    /// Rows shown last time (for scrolling limits).
    rows: Cell<usize>,
    /// Command run by `r`; `None` for a group.
    run: Option<String>,
}

impl HelpView {
    /// The help page of `topic`.
    pub fn new(commands: &Commands, topic: &Topic) -> Self {
        let (name, category, body, run) = match topic {
            Topic::Command(name) => {
                let c = commands.get(name).expect("resolved");
                (
                    name.clone(),
                    c.category.clone(),
                    command_body(commands, c),
                    Some(name.clone()),
                )
            }
            Topic::Group(root) => (
                root.clone(),
                group_category(commands, root),
                vec![(Style::default(), format!("Commands under :{root}."), 0)],
                None,
            ),
        };
        let mut path = vec!["Commands".to_string()];
        if !category.is_empty() {
            path.push(category);
        }
        path.extend(name.split(' ').map(String::from));
        Self {
            title: format!(":help {name}"),
            path: path.join(" › "),
            body,
            subs: subcommands(commands, &name),
            selected: None,
            top: 0,
            rows: Cell::new(0),
            run,
        }
    }

    /// The breadcrumb path, e.g. `Commands › Plugins › plugin › add`.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Full name of the selected subcommand.
    pub fn selected(&self) -> Option<&str> {
        self.selected
            .and_then(|i| self.subs.get(i))
            .map(|(n, _)| n.as_str())
    }

    /// Footer with the keys that work here.
    fn hint(&self) -> String {
        let mut parts = Vec::new();
        if self.subs.is_empty() {
            parts.push("j/k scroll");
        } else {
            parts.push("j/k select · Enter open");
        }
        if self.run.is_some() {
            parts.push("r run");
        }
        parts.push("Esc back");
        parts.join(" · ")
    }

    /// All lines: body, then the subcommand list. The second value is the line index of
    /// the first subcommand.
    fn lines(&self) -> (Vec<Line>, usize) {
        let mut lines = self.body.clone();
        if self.subs.is_empty() {
            let n = lines.len();
            return (lines, n);
        }
        lines.push((Style::default(), String::new(), 0));
        lines.push((Style::BOLD, "Subcommands".into(), 0));
        let first = lines.len();
        let w = self
            .subs
            .iter()
            .map(|(n, _)| n.chars().count())
            .max()
            .unwrap_or(0);
        for (name, desc) in &self.subs {
            lines.push((Style::default(), format!("  {name:<w$}  {desc}"), w + 4));
        }
        (lines, first)
    }
}

/// Text of a command's help page, above its subcommands.
fn command_body(commands: &Commands, c: &Command) -> Vec<Line> {
    let plain = Style::default();
    let head = Style::BOLD;
    let blank = || (Style::default(), String::new(), 0);
    let row = |label: &str, text: String| (plain, format!("{label:<LABEL$}{text}"), LABEL);
    let mut out = vec![(plain, c.description.clone(), 0), blank()];
    out.push(row("Usage", format!(":{}", c.usage())));
    let aliases: Vec<String> = commands
        .aliases_of(&c.name)
        .map(|a| format!(":{a}"))
        .collect();
    if !aliases.is_empty() {
        out.push(row("Aliases", aliases.join(", ")));
    }
    out.push(row("Category", c.category.clone()));
    out.push(row("From", c.owner.clone()));
    if !c.params.is_empty() {
        out.push(blank());
        out.push((head, "Parameters".into(), 0));
        let w = c
            .params
            .iter()
            .map(|p| p.name.chars().count())
            .max()
            .unwrap_or(0);
        for p in &c.params {
            let mut text = p.description.clone();
            if let Complete::Values(v) = &p.complete {
                text.push_str(&format!(" One of: {}.", v.join(", ")));
            }
            let need = match (p.required, p.repeated) {
                (true, false) => "required",
                (true, true) => "1 or more",
                (false, false) => "optional",
                (false, true) => "0 or more",
            };
            out.push((
                plain,
                format!("  {:<w$}  {need:<9}  {text}", p.name),
                w + 15,
            ));
        }
    }
    if let Some(f) = &c.force {
        out.push(blank());
        out.push((head, format!("With ! (:{}!)", c.name), 0));
        out.push((plain, format!("  {f}"), 2));
    }
    if let Some(r) = &c.returns {
        out.push(blank());
        out.push((head, "Result".into(), 0));
        out.push((plain, format!("  {r}"), 2));
    }
    if !c.examples.is_empty() {
        out.push(blank());
        out.push((head, "Examples".into(), 0));
        let w = c
            .examples
            .iter()
            .map(|e| e.line.chars().count())
            .max()
            .unwrap_or(0)
            + 1;
        for e in &c.examples {
            out.push((
                plain,
                format!("  {:<w$}  {}", format!(":{}", e.line), e.note),
                w + 4,
            ));
        }
    }
    out
}

impl View for HelpView {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth) {
        if area.h == 0 {
            return;
        }
        s.put_in(area, 0, &self.path, Style::BOLD.fg(Color::Blue), width);
        // Rows between the path (plus a blank row) and the footer.
        let text_h = area.h.saturating_sub(3) as usize;
        let (lines, first_sub) = self.lines();
        let mut rows: Vec<(Style, String)> = Vec::new();
        let mut selected_row = None;
        for (i, (style, text, indent)) in lines.iter().enumerate() {
            let sel = self.selected.is_some_and(|s| first_sub + s == i);
            if sel {
                selected_row = Some(rows.len());
            }
            let style = if sel { Style::REVERSED } else { *style };
            for r in wrap(text, area.w as usize, *indent, width) {
                rows.push((style, r));
            }
        }
        self.rows.set(rows.len());
        let max_top = rows.len().saturating_sub(text_h);
        let top = match selected_row {
            Some(r) if r >= text_h => (r + 1 - text_h).min(max_top),
            Some(_) => 0,
            None => self.top.min(max_top),
        };
        for (i, (style, text)) in rows.iter().skip(top).take(text_h).enumerate() {
            let line = Rect::new(area.x, area.y + 2 + i as u16, area.w, 1);
            if style.reversed {
                s.clear(line, *style);
            }
            s.put_in(line, 0, text, *style, width);
        }
        let foot = Rect::new(area.x, area.y + area.h - 1, area.w, 1);
        s.put_in(foot, 0, &self.hint(), Style::BOLD.fg(Color::Blue), width);
    }

    fn handle_key(&mut self, key: Key) -> Outcome {
        let last = self.subs.len().checked_sub(1);
        match (key.code, last) {
            (KeyCode::Char('j') | KeyCode::Down, Some(last)) => {
                self.selected = Some(self.selected.map_or(0, |s| (s + 1).min(last)));
            }
            (KeyCode::Char('k') | KeyCode::Up, Some(_)) => {
                self.selected = self.selected.and_then(|s| s.checked_sub(1));
            }
            (KeyCode::Char('j') | KeyCode::Down, None) => {
                let max = self.rows.get().saturating_sub(1);
                self.top = (self.top + 1).min(max);
            }
            (KeyCode::Char('k') | KeyCode::Up, None) => self.top = self.top.saturating_sub(1),
            (KeyCode::Char('g') | KeyCode::Home, _) => {
                self.selected = None;
                self.top = 0;
            }
            (KeyCode::Char('G') | KeyCode::End, Some(last)) => self.selected = Some(last),
            (KeyCode::Enter, _) => {
                return match self.selected() {
                    Some(name) => Outcome::RunAndClose(format!("help {name}")),
                    None => Outcome::Handled,
                };
            }
            (KeyCode::Char('r'), _) => {
                return match &self.run {
                    Some(name) => Outcome::RunAndClose(format!("commands run {name}")),
                    None => Outcome::Handled,
                };
            }
            (KeyCode::Char('q') | KeyCode::Esc, _) => return Outcome::Close,
            _ => return Outcome::Ignored,
        }
        Outcome::Handled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> Key {
        crate::keys::parse_keys(s)[0]
    }

    fn commands() -> Commands {
        let mut c = Commands::default();
        c.register("plugin add", "Install a plugin.", |_, _| Ok(()))
            .category("Plugins")
            .param(Param::required("SOURCE", "Folder or git URL."));
        c.register("plugin remove", "Uninstall.", |_, _| Ok(()))
            .category("Plugins")
            .force("Do not ask.");
        c.register("write", "Save.", |_, _| Ok(()));
        c.alias("w", "write");
        c
    }

    #[test]
    fn resolve_topics() {
        let c = commands();
        assert_eq!(resolve(&c, "w"), Some(Topic::Command("write".into())));
        assert_eq!(resolve(&c, ":w!"), Some(Topic::Command("write".into())));
        assert_eq!(
            resolve(&c, " plugin   remove! "),
            Some(Topic::Command("plugin remove".into()))
        );
        assert_eq!(resolve(&c, "plugin"), Some(Topic::Group("plugin".into())));
        assert_eq!(resolve(&c, "plug"), None);
        assert_eq!(resolve(&c, ""), None);
    }

    #[test]
    fn view_path_and_navigation() {
        let c = commands();
        let mut v = HelpView::new(&c, &Topic::Group("plugin".into()));
        assert_eq!(v.path(), "Commands › Plugins › plugin");
        assert_eq!(v.handle_key(key("k")), Outcome::Handled);
        assert_eq!(v.selected(), None);
        assert_eq!(v.handle_key(key("<CR>")), Outcome::Handled);
        for _ in 0..3 {
            v.handle_key(key("j"));
        }
        assert_eq!(v.selected(), Some("plugin remove"));
        assert_eq!(
            v.handle_key(key("<CR>")),
            Outcome::RunAndClose("help plugin remove".into())
        );
        assert_eq!(v.handle_key(key("r")), Outcome::Handled);
        assert_eq!(v.handle_key(key("<Esc>")), Outcome::Close);
        let mut v = HelpView::new(&c, &Topic::Command("plugin add".into()));
        assert_eq!(
            v.handle_key(key("r")),
            Outcome::RunAndClose("commands run plugin add".into())
        );
    }
}
