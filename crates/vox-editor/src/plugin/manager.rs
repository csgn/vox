//! The plugin manager (`:plugins`, `<leader>p`): every plugin of the active scope with its
//! status, version, and source, and keys to add, configure, turn on/off, delete, reload,
//! check, and sync plugins. Opens as a modal or in the side panel (`[ui] plugin_manager`).
//! Every action is a command, so it can also be mapped or typed.

use std::cell::Cell;

use vox_view::CharWidth;

use super::health::HealthStatus;
use super::install::{self, LockState};
use super::lock::{self, Source};
use crate::Editor;
use crate::keys::{Key, KeyCode};
use crate::scope::ScopeKind;
use crate::surface::{Color, Rect, Style, Surface};
use crate::ui::{Level, Outcome, View};

/// Title prefix (used to find an open manager).
pub const TITLE: &str = "Plugins";

/// Rows at the bottom: details (3) and key hints (1).
const FOOTER: u16 = 4;

/// One plugin row.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    name: String,
    version: String,
    /// `ok`, `warning`, `failed`, `disabled`, `missing`, ...
    status: String,
    level: Option<Level>,
    source: String,
    description: String,
    /// First problem, if any.
    problem: Option<String>,
    /// What it adds: commands and keys.
    adds: String,
    settings: usize,
    disabled: bool,
}

/// The plugin manager view.
pub struct ManagerView {
    rows: Vec<Row>,
    selected: Cell<usize>,
    /// Shown in the title: scope and folder.
    place: String,
    kind: ScopeKind,
    /// Open as a modal (dialogs it opens go back to it) or in the side panel.
    in_modal: bool,
}

impl ManagerView {
    /// A manager for `ed`'s active scope.
    pub fn new(ed: &Editor, in_modal: bool) -> Self {
        let mut v = Self {
            rows: Vec::new(),
            selected: Cell::new(0),
            place: String::new(),
            kind: ed.scope().kind,
            in_modal,
        };
        v.refresh(ed);
        v
    }

    /// The selected row.
    fn row(&self) -> Option<&Row> {
        self.rows.get(self.selected.get())
    }

    /// Run `cmd`, which opens a dialog: from a modal manager, the dialog goes back to it.
    fn dialog(&self, cmd: String) -> Outcome {
        if self.in_modal {
            Outcome::RunAndClose(cmd)
        } else {
            Outcome::Run(cmd)
        }
    }

    /// `cmd` with the selected plugin's name, if there is a selection.
    fn on_row(&self, f: impl Fn(&Row) -> Outcome) -> Outcome {
        self.row().map_or(Outcome::Handled, f)
    }
}

/// `git: URL@REF`, `path: P`, or `not in vox.lock`.
fn describe_source(s: Option<&Source>) -> String {
    match s {
        Some(Source::Git { url, reference, .. }) => match reference {
            Some(r) => format!("git {url}@{r}"),
            None => format!("git {url}"),
        },
        Some(Source::Path(p)) => format!("path {p}"),
        None => "not in vox.lock".into(),
    }
}

impl View for ManagerView {
    fn title(&self) -> String {
        format!("{TITLE} — {}", self.place)
    }

    /// Rebuild the rows from the plugin host's infos and the lockfile.
    fn refresh(&mut self, ed: &Editor) {
        let scope = ed.scope();
        self.kind = scope.kind;
        self.place = match (&scope.kind, scope.plugin_dir()) {
            (ScopeKind::Vanilla, _) | (_, None) => "vanilla: plugins are off".into(),
            (k, Some(dir)) => format!("{}: {}", k.name(), dir.display()),
        };
        let lockfile = match (ed.file_format(), scope.lock_path()) {
            (Some((parse, _)), Some(path)) => lock::read(&path, parse).unwrap_or_default(),
            _ => Default::default(),
        };
        let mut rows: Vec<Row> = ed
            .plugin_infos()
            .iter()
            .map(|info| {
                let owner = crate::owner::plugin(&info.name);
                let cmds: Vec<String> = ed
                    .commands()
                    .iter()
                    .filter(|c| c.owner == owner)
                    .map(|c| format!(":{}", c.name))
                    .collect();
                let keys: Vec<String> = ed
                    .keymap()
                    .iter()
                    .filter(|m| m.3 == owner)
                    .map(|m| crate::keys::format_keys(m.1))
                    .collect();
                let adds = [cmds.join(" "), keys.join(" ")]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                let level = match info.status {
                    HealthStatus::Failed => Some(Level::Error),
                    HealthStatus::Warning => Some(Level::Warn),
                    _ => None,
                };
                Row {
                    name: info.name.clone(),
                    version: info.version().map_or("—".into(), |v| v.to_string()),
                    status: info.status.name().into(),
                    level,
                    source: describe_source(lockfile.get(&info.name).map(|e| &e.source)),
                    description: info
                        .manifest
                        .as_ref()
                        .map(|m| m.description.clone())
                        .unwrap_or_default(),
                    problem: info.items.iter().find(|i| !i.ok).map(|i| i.message.clone()),
                    adds,
                    settings: info.manifest.as_ref().map_or(0, |m| m.settings.len()),
                    disabled: info.status == HealthStatus::Disabled,
                }
            })
            .collect();
        // Locked but not installed: shown so they can be synced.
        if let Some(dir) = scope.plugin_dir() {
            for (name, state) in install::check_lock(&lockfile, &dir) {
                if state == LockState::Missing && !rows.iter().any(|r| r.name == name) {
                    let e = lockfile.get(&name).expect("from the lock");
                    rows.push(Row {
                        name: name.clone(),
                        version: e.version.clone(),
                        status: "missing".into(),
                        level: Some(Level::Warn),
                        source: describe_source(Some(&e.source)),
                        description: String::new(),
                        problem: Some("in vox.lock but not installed: s to sync".into()),
                        adds: String::new(),
                        settings: 0,
                        disabled: false,
                    });
                }
            }
        }
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        self.rows = rows;
        let last = self.rows.len().saturating_sub(1);
        self.selected.set(self.selected.get().min(last));
    }

    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth) {
        let line = |y: u16| Rect::new(area.x, area.y + y, area.w, 1);
        let list_h = area.h.saturating_sub(FOOTER);
        if self.kind == ScopeKind::Vanilla {
            s.put_in(
                line(0),
                0,
                "Plugins are off in vanilla mode.",
                Style::default(),
                width,
            );
            s.put_in(
                line(1),
                0,
                "Start `vox` (global) or `vox --local-config` (project).",
                Style::DIM,
                width,
            );
            return;
        }
        if self.rows.is_empty() {
            s.put_in(
                line(0),
                0,
                "No plugins yet. Press a to add one.",
                Style::DIM,
                width,
            );
        }
        let sel = self.selected.get();
        let top = sel.saturating_sub((list_h as usize).saturating_sub(1));
        let name_w = self
            .rows
            .iter()
            .map(|r| r.name.chars().count())
            .max()
            .unwrap_or(0);
        for (i, r) in self.rows.iter().enumerate().skip(top).take(list_h as usize) {
            let y = (i - top) as u16;
            let mut style = if i == sel {
                Style::REVERSED
            } else {
                Style::default()
            };
            if r.disabled {
                style.dim = true;
            }
            s.clear(line(y), style);
            let badge = match r.status.as_str() {
                "ok" | "info" => "✓",
                "warning" | "missing" => "!",
                "failed" => "✗",
                _ => "–",
            };
            let badge_style = match (i == sel, r.level) {
                (false, Some(l)) => Style {
                    fg: l.color(),
                    ..style
                },
                (false, None) if !r.disabled => Style {
                    fg: Color::Green,
                    ..style
                },
                _ => style,
            };
            s.put_in(line(y), 0, &format!(" {badge} "), badge_style, width);
            let text = format!(
                "{:<name_w$}  {:<8} {:<9} {}",
                r.name, r.version, r.status, r.source
            );
            let rest = Rect::new(area.x + 3, area.y + y, area.w.saturating_sub(3), 1);
            s.put_in(rest, 0, &text, style, width);
        }
        let foot = list_h;
        if let Some(r) = self.row() {
            let desc = if r.description.is_empty() {
                "(no description)"
            } else {
                &r.description
            };
            s.put_in(
                line(foot),
                0,
                &format!("{}: {desc}", r.name),
                Style::BOLD,
                width,
            );
            match &r.problem {
                Some(p) => {
                    let color = r.level.map_or(Color::Blue, |l| l.color());
                    s.put_in(line(foot + 1), 0, p, Style::default().fg(color), width);
                }
                None => s.put_in(
                    line(foot + 1),
                    0,
                    "healthy",
                    Style::default().fg(Color::Green),
                    width,
                ),
            }
            let info = format!(
                "{}{} setting(s)",
                if r.adds.is_empty() {
                    String::new()
                } else {
                    format!("adds {} · ", r.adds)
                },
                r.settings
            );
            s.put_in(line(foot + 2), 0, &info, Style::DIM, width);
        }
        let hint = "a add · e settings · E files · Space on/off · D delete · r reload · h health · s sync · ? help";
        s.put_in(line(foot + 3), 0, hint, Style::BOLD.fg(Color::Blue), width);
    }

    fn handle_key(&mut self, key: Key) -> Outcome {
        if self.kind == ScopeKind::Vanilla {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('q') => Outcome::Close,
                _ => Outcome::Handled,
            };
        }
        let last = self.rows.len().saturating_sub(1);
        let sel = self.selected.get();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.selected.set((sel + 1).min(last)),
            KeyCode::Char('k') | KeyCode::Up => self.selected.set(sel.saturating_sub(1)),
            KeyCode::Char('g') | KeyCode::Home => self.selected.set(0),
            KeyCode::Char('G') | KeyCode::End => self.selected.set(last),
            KeyCode::Char('a') => return self.dialog("plugin add".into()),
            KeyCode::Char('e') | KeyCode::Enter => {
                return self.on_row(|r| self.dialog(format!("settings plugin {}", r.name)));
            }
            KeyCode::Char('E') => {
                return self.on_row(|r| self.dialog(format!("plugin files {}", r.name)));
            }
            KeyCode::Char(' ') => {
                return self.on_row(|r| {
                    let verb = if r.disabled { "enable" } else { "disable" };
                    Outcome::Run(format!("plugin {verb} {}", r.name))
                });
            }
            KeyCode::Char('D') => {
                return self.on_row(|r| self.dialog(format!("plugin remove {}", r.name)));
            }
            KeyCode::Char('r') => {
                return self.on_row(|r| Outcome::Run(format!("reload plugins {}", r.name)));
            }
            KeyCode::Char('h') => {
                return self.on_row(|r| self.dialog(format!("checkhealth {}", r.name)));
            }
            KeyCode::Char('s') => return Outcome::Run("plugins sync".into()),
            KeyCode::Char('c') if self.kind == ScopeKind::Local => {
                return self.dialog("scope pick".into());
            }
            KeyCode::Char('?') => return self.dialog("plugins help".into()),
            KeyCode::Esc | KeyCode::Char('q') => return Outcome::Close,
            _ => return Outcome::Ignored,
        }
        Outcome::Handled
    }
}

/// Key help for the manager.
pub const HELP: &[(&str, &str)] = &[
    ("j / k", "move"),
    (
        "a",
        "add a plugin: a folder, or a git URL (optionally @tag/branch/commit)",
    ),
    ("e, Enter", "edit the plugin's settings"),
    ("E", "open the plugin's files (plugin.toml)"),
    ("Space", "turn the plugin on / off (saved in config.toml)"),
    ("D", "delete the plugin (asks first)"),
    ("r", "reload the plugin"),
    ("h", "health report"),
    ("s", "install what vox.lock lists (sync)"),
    ("c", "copy plugins from your global setup (project mode)"),
    ("q, Esc", "close"),
];
