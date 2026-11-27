//! Editor state and key handling.

use vox_core::storage::{SimpleStorage, StorageFactory};
use vox_core::{Document, Position, motion, search};
use vox_view::{CharWidth, Monospace, Viewport, display_col};

use std::time::Duration;

use crate::builtin;
use crate::command::{self, Commands};
use std::time::Instant;

use crate::config::Config;
use crate::event::{Event, EventHandler, EventKind, ModeKind};
use crate::keymap::{self, Binding, Keymap, Lookup, MapMode};
use crate::keys::{Key, KeyCode};
use crate::log::Log;
use crate::owner;
use crate::scope::Scope;
use crate::ui::{
    self, Focus, Layout, Level, Notification, Outcome, PanelSlot, SidePosition, UiConfig, View,
};

use crate::normal::{self, Action, Cmd, InsertAt, Motion, Operator, Parse, Scroll, Target};
use crate::register::{self, Clipboard, NoClipboard, Reg, Registers};

/// Editor input mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Navigation and commands.
    Normal,
    /// Text entry.
    Insert,
    /// Selection from the anchor to the cursor; `line` selects whole lines.
    Visual {
        /// Linewise (`V`) instead of charwise (`v`).
        line: bool,
    },
    /// `:` command line; holds the text typed so far.
    Command(String),
    /// `/` search prompt; holds the text typed so far.
    Search(String),
}

/// How a motion's range is interpreted by an operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Exclusive,
    Inclusive,
    Linewise,
}

/// Text an operator acts on. For linewise ranges only the lines of `start`/`end` matter.
#[derive(Debug, Clone, Copy)]
struct Range {
    start: Position,
    /// Exclusive end (charwise) or last line (linewise).
    end: Position,
    linewise: bool,
}

/// Platform services the editor uses. Adapters provide real implementations; the
/// defaults are std-only stand-ins suitable for tests.
pub struct Services {
    /// Creates text storage for documents opened with `:e`.
    pub storage: StorageFactory,
    /// Terminal cell width of chars.
    pub width: Box<dyn CharWidth>,
    /// Backing store of the `"+` / `"*` registers.
    pub clipboard: Box<dyn Clipboard>,
    /// Current time; replaced in tests to control notification expiry.
    pub clock: Box<dyn Fn() -> Instant>,
}

impl Default for Services {
    fn default() -> Self {
        Self {
            storage: SimpleStorage::boxed,
            width: Box::new(Monospace),
            clipboard: Box::new(NoClipboard),
            clock: Box::new(Instant::now),
        }
    }
}

/// Top-level editor state.
pub struct Editor {
    pub(crate) doc: Document,
    pub(crate) view: Viewport,
    pub(crate) cursor: Position,
    /// Column `j`/`k` try to keep.
    want_col: usize,
    pub(crate) mode: Mode,
    /// Visual mode anchor.
    anchor: Position,
    /// Terminal size (width, height), updated each frame.
    pub(crate) size: (u16, u16),
    /// Keys of an incomplete normal/visual command.
    pending: Vec<Key>,
    registers: Registers,
    last_search: Option<String>,
    pub(crate) message: Option<String>,
    quit: bool,
    storage: StorageFactory,
    pub(crate) width: Box<dyn CharWidth>,
    commands: Commands,
    keymap: Keymap,
    /// Typed keys that may still become a mapping.
    map_pending: Vec<Key>,
    key_timeout: Duration,
    /// What `<leader>` means in mappings.
    leader: String,
    /// (kind, owner, handler).
    subscribers: Vec<(EventKind, String, EventHandler)>,
    /// Owner given to new commands, mappings, and subscriptions (see [`crate::owner`]).
    owner: String,
    log: Log,
    developer_mode: bool,
    /// Messages from background jobs.
    jobs_tx: std::sync::mpsc::Sender<JobMsg>,
    jobs_rx: std::sync::mpsc::Receiver<JobMsg>,
    jobs_running: usize,
    /// Jobs shown on screen while they run (not quiet ones).
    job_status: Vec<ui::JobStatus>,
    /// Id for the next job.
    next_job_id: u64,
    /// Reads and writes plugin/lock files (from an adapter).
    file_format: Option<(crate::config::ValueParser, crate::config::ValueFormatter)>,
    /// The config last applied (what the settings dialog edits).
    config: Config,
    /// Writes edits into the scope's config file (from an adapter).
    config_writer: Option<crate::settings::ConfigWriter>,
    /// Plugins known to the plugin host, for the plugin manager.
    plugin_infos: Vec<crate::plugin::PluginInfo>,
    /// Settings each plugin declares in its manifest.
    plugin_settings: std::collections::BTreeMap<String, Vec<crate::plugin::SettingSpec>>,
    /// `[plugins.NAME]` values from the config.
    plugin_config: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, crate::config::Value>,
    >,
    /// Active scope, and the global one (source for copying into a project).
    scope: Scope,
    global_scope: Option<Scope>,
    ui: UiConfig,
    side: Option<Box<dyn View>>,
    bottom: Option<Box<dyn View>>,
    modal: Option<Box<dyn View>>,
    /// Modals the current one was opened from, oldest first; `Esc` goes back to the last.
    modal_history: Vec<Box<dyn View>>,
    /// Values of the last submitted form (see [`Editor::take_submitted`]).
    submitted: Vec<(String, crate::config::Value)>,
    focus: Focus,
    /// `<C-w>` typed while a panel has focus; the next key picks the direction.
    ctrl_w_pending: bool,
    notifications: Vec<Notification>,
    /// Messages and notifications, oldest first (`:messages`).
    history: Vec<(Level, String)>,
    clock: Box<dyn Fn() -> Instant>,
    /// When the first still-pending mapping key was typed.
    pending_since: Option<Instant>,
    /// Nesting depth of public entry points; only the outermost sends change events.
    event_depth: u32,
    /// A command ran since open views were last refreshed.
    views_stale: bool,
    /// A person is at the keyboard (see [`Editor::interactive`]).
    interactive: bool,
    /// Data returned by the running command (see [`Editor::reply`]).
    reply: Option<crate::config::Value>,
    /// The running command tried to open a view while not interactive.
    view_refused: bool,
    /// Where vox updates come from (set by the binary).
    update_source: Option<crate::update::UpdateSource>,
    /// vox version installed by `:update install` since start (runs after a restart).
    update_installed: Option<crate::plugin::Version>,
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field("doc", &self.doc)
            .field("cursor", &self.cursor)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl Editor {
    /// Create an editor for `doc` with default (std-only) [`Services`].
    pub fn new(doc: Document) -> Self {
        Self::with_services(doc, Services::default())
    }

    /// Create an editor for `doc` using `services`.
    pub fn with_services(doc: Document, services: Services) -> Self {
        let mut registers = Registers::default();
        registers.clipboard = services.clipboard;
        let (jobs_tx, jobs_rx) = std::sync::mpsc::channel();
        let mut commands = Commands::default();
        commands.owner = owner::BUILTIN.into();
        builtin::register(&mut commands);
        commands.owner = owner::USER.into();
        let mut ed = Self {
            doc,
            view: Viewport::default(),
            cursor: Position::default(),
            want_col: 0,
            mode: Mode::Normal,
            anchor: Position::default(),
            size: (80, 24),
            pending: Vec::new(),
            registers,
            last_search: None,
            message: None,
            quit: false,
            storage: services.storage,
            width: services.width,
            commands,
            keymap: Keymap::default(),
            map_pending: Vec::new(),
            key_timeout: Duration::from_millis(1000),
            leader: "\\".into(),
            subscribers: Vec::new(),
            owner: owner::USER.into(),
            log: Log::new((services.clock)()),
            developer_mode: false,
            plugin_config: Default::default(),
            config: Config::default(),
            config_writer: None,
            jobs_tx,
            jobs_rx,
            jobs_running: 0,
            job_status: Vec::new(),
            next_job_id: 0,
            file_format: None,
            plugin_settings: Default::default(),
            plugin_infos: Vec::new(),
            // Until the binary sets one: global without a folder (no badge, nothing loaded).
            scope: Scope {
                kind: crate::scope::ScopeKind::Global,
                dir: None,
            },
            global_scope: None,
            ui: UiConfig::default(),
            side: None,
            bottom: None,
            modal: None,
            modal_history: Vec::new(),
            submitted: Vec::new(),
            focus: Focus::Editor,
            ctrl_w_pending: false,
            notifications: Vec::new(),
            history: Vec::new(),
            clock: services.clock,
            pending_since: None,
            event_depth: 0,
            views_stale: false,
            interactive: true,
            reply: None,
            view_refused: false,
            update_source: None,
            update_installed: None,
        };
        ed.default_mappings();
        ed
    }

    /// Replace the clipboard used by the `"+` and `"*` registers.
    pub fn set_clipboard(&mut self, clipboard: Box<dyn Clipboard>) {
        self.registers.clipboard = clipboard;
    }

    /// Whether the app has been asked to quit.
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Current mode.
    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    /// The document being edited.
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// Cursor position.
    pub fn cursor(&self) -> Position {
        self.cursor
    }

    /// Current viewport.
    pub fn viewport(&self) -> Viewport {
        self.view
    }

    /// Last message shown on the command line, if any.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Visual selection as (start, end inclusive, linewise), if in visual mode.
    pub fn selection(&self) -> Option<(Position, Position, bool)> {
        let Mode::Visual { line } = self.mode else {
            return None;
        };
        let (a, b) = order(self.anchor, self.cursor);
        Some((a, b, line))
    }

    /// Update the terminal size and keep the cursor visible.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.size = (width, height);
        self.follow_cursor();
    }

    /// Width of the line-number gutter including its trailing space.
    pub(crate) fn gutter_width(&self) -> usize {
        self.doc.line_count().to_string().len().max(3) + 1
    }

    /// Text area size (width, height) in cells, excluding the gutter.
    pub(crate) fn text_area(&self) -> (usize, usize) {
        let area = self.layout().editor;
        let width = (area.w as usize).saturating_sub(self.gutter_width()).max(1);
        (width, (area.h as usize).max(1))
    }

    /// Where everything goes on screen right now.
    pub(crate) fn layout(&self) -> Layout {
        ui::compute(
            self.size,
            &self.ui,
            self.side.is_some(),
            self.bottom.is_some(),
            self.modal.is_some(),
        )
    }

    /// Display column of the cursor within its line.
    pub(crate) fn cursor_dcol(&self) -> usize {
        display_col(
            self.width.as_ref(),
            &self.doc.line(self.cursor.line).unwrap_or_default(),
            self.cursor.col,
        )
    }

    /// Scroll the viewport just enough to keep the cursor on screen.
    fn follow_cursor(&mut self) {
        let (w, h) = self.text_area();
        self.view.follow(self.cursor.line, self.cursor_dcol(), w, h);
    }

    /// Handle one key press.
    pub fn handle_key(&mut self, key: Key) {
        self.with_change_events(|ed| {
            if ed.modal.is_some() {
                ed.view_key(None, key);
                return;
            }
            let prompt = matches!(ed.mode, Mode::Command(_) | Mode::Search(_));
            if let (Some(slot), false) = (ed.focused_panel(), prompt) {
                ed.panel_key(slot, key);
                return;
            }
            ed.map_pending.push(key);
            ed.resolve_mappings(false);
        });
        if self.views_stale {
            self.refresh_views();
        }
        self.pending_since = match (self.map_pending.is_empty(), self.pending_since) {
            (true, _) => None,
            (false, None) => Some((self.clock)()),
            (false, since) => since,
        };
        self.follow_cursor();
    }

    /// Key for a focused panel: `<C-w>` window keys and `:` work as usual; the rest goes
    /// to the panel's view.
    fn panel_key(&mut self, slot: PanelSlot, key: Key) {
        if std::mem::take(&mut self.ctrl_w_pending) {
            if let KeyCode::Char(c) = key.code {
                self.window_cmd(c);
            }
            return;
        }
        if key == Key::ctrl('w') {
            self.ctrl_w_pending = true;
            return;
        }
        if key.code == KeyCode::Char(':') && !key.mods.ctrl {
            self.message = None;
            self.mode = Mode::Command(String::new());
            return;
        }
        self.view_key(Some(slot), key);
    }

    /// Give `key` to the modal (`None`) or a panel, then act on the view's [`Outcome`].
    fn view_key(&mut self, slot: Option<PanelSlot>, key: Key) {
        let taken = match slot {
            None => self.modal.take(),
            Some(PanelSlot::Side) => self.side.take(),
            Some(PanelSlot::Bottom) => self.bottom.take(),
        };
        let Some(mut view) = taken else {
            return;
        };
        let outcome = view.handle_key(key);
        if slot.is_none() {
            self.modal_outcome(view, outcome);
            return;
        }
        let keep = !matches!(
            outcome,
            Outcome::Close | Outcome::RunAndClose(_) | Outcome::Submit { .. }
        );
        if keep {
            self.put_view(slot, view);
        } else if slot.is_some_and(|s| self.focus == focus_of(s)) {
            self.focus = Focus::Editor;
        }
        match outcome {
            Outcome::Run(cmd) | Outcome::RunAndClose(cmd) => self.run_command(&cmd),
            Outcome::Submit { command, values } => {
                self.submitted = values;
                self.run_command(&command);
            }
            _ => {}
        }
    }

    /// Values from the last [`Outcome::Submit`] (e.g. a saved form), taken once by the
    /// command it ran.
    pub fn take_submitted(&mut self) -> Vec<(String, crate::config::Value)> {
        std::mem::take(&mut self.submitted)
    }

    /// Act on a modal's outcome. A command that opens another modal keeps this one in the
    /// history, so closing the new one (`Esc`/`q`) goes back here.
    fn modal_outcome(&mut self, view: Box<dyn View>, outcome: Outcome) {
        match outcome {
            Outcome::Handled | Outcome::Ignored => self.modal = Some(view),
            Outcome::Run(cmd) => {
                self.modal = Some(view);
                self.run_command(&cmd);
            }
            Outcome::Close => self.modal = self.modal_history.pop(),
            Outcome::Submit { command, values } => {
                // Back to where the form was opened from, then save.
                self.modal = self.modal_history.pop();
                self.submitted = values;
                self.run_command(&command);
            }
            Outcome::RunAndClose(cmd) => {
                let depth = self.modal_history.len();
                self.run_command(&cmd);
                if self.modal.is_some() {
                    // The command opened a new modal: remember where we came from.
                    self.modal_history.insert(depth, view);
                } else {
                    // A final choice: the whole chain of dialogs is done.
                    self.modal_history.clear();
                }
            }
        }
    }

    /// How many modals are below the current one (each `Esc` goes back one).
    pub fn modal_depth(&self) -> usize {
        self.modal_history.len()
    }

    /// Store `view` in the modal (`None`) or a panel slot.
    fn put_view(&mut self, slot: Option<PanelSlot>, view: Box<dyn View>) {
        match slot {
            None => self.modal = Some(view),
            Some(PanelSlot::Side) => self.side = Some(view),
            Some(PanelSlot::Bottom) => self.bottom = Some(view),
        }
    }

    /// The panel that has focus, if it is open.
    fn focused_panel(&self) -> Option<PanelSlot> {
        match self.focus {
            Focus::Side if self.side.is_some() => Some(PanelSlot::Side),
            Focus::Bottom if self.bottom.is_some() => Some(PanelSlot::Bottom),
            _ => None,
        }
    }

    /// `<C-w>` + `c`: `h`/`l` left/right, `j`/`k` down/up, `w` next, `c`/`q` close panel.
    pub(crate) fn window_cmd(&mut self, c: char) {
        let side_left = self.ui.side_panel == SidePosition::Left;
        let target = match (c, self.focus) {
            ('h', Focus::Editor) if side_left => Focus::Side,
            ('h', Focus::Side) if !side_left => Focus::Editor,
            ('l', Focus::Editor) if !side_left => Focus::Side,
            ('l', Focus::Side) if side_left => Focus::Editor,
            ('j', Focus::Editor | Focus::Side) => Focus::Bottom,
            ('k', Focus::Bottom) => Focus::Editor,
            ('w', Focus::Editor) if self.side.is_some() => Focus::Side,
            ('w', Focus::Editor | Focus::Side) if self.bottom.is_some() => Focus::Bottom,
            ('w', _) => Focus::Editor,
            ('c' | 'q', _) => {
                self.close_focused();
                return;
            }
            _ => return,
        };
        self.focus(target);
    }

    /// Move focus to `target` if it is open (the editor is always open).
    pub fn focus(&mut self, target: Focus) {
        let open = match target {
            Focus::Editor => true,
            Focus::Side => self.side.is_some(),
            Focus::Bottom => self.bottom.is_some(),
        };
        if open {
            self.focus = target;
        }
    }

    /// Which part has focus (ignoring the modal, which always has focus while open).
    pub fn focused(&self) -> Focus {
        self.focus
    }

    /// Show `view` in the modal (replacing any open one). The modal takes all keys.
    /// Not interactive (see [`Editor::interactive`]): nothing opens and the command fails.
    pub fn open_modal(&mut self, view: Box<dyn View>) {
        if !self.interactive {
            self.view_refused = true;
            return;
        }
        self.modal = Some(view);
    }

    /// Show `view` in a panel (replacing what was there) and focus it.
    /// Not interactive (see [`Editor::interactive`]): nothing opens and the command fails.
    pub fn open_panel(&mut self, slot: PanelSlot, view: Box<dyn View>) {
        if !self.interactive {
            self.view_refused = true;
            return;
        }
        self.put_view(Some(slot), view);
        self.focus = focus_of(slot);
    }

    /// Close the modal.
    /// Closes the whole chain of modals (see [`Editor::modal_depth`]).
    pub fn close_modal(&mut self) {
        self.modal = None;
        self.modal_history.clear();
    }

    /// Close a panel; focus returns to the editor if it was there.
    pub fn close_panel(&mut self, slot: PanelSlot) {
        match slot {
            PanelSlot::Side => self.side = None,
            PanelSlot::Bottom => self.bottom = None,
        }
        if self.focus == focus_of(slot) {
            self.focus = Focus::Editor;
        }
    }

    /// Close the modal, else the focused panel, else the bottom panel, else the side panel.
    pub fn close_focused(&mut self) {
        if self.modal.is_some() {
            self.close_modal();
        } else if let Some(slot) = self.focused_panel() {
            self.close_panel(slot);
        } else if self.bottom.is_some() {
            self.close_panel(PanelSlot::Bottom);
        } else {
            self.close_panel(PanelSlot::Side);
        }
    }

    /// Whether the modal is open.
    pub fn has_modal(&self) -> bool {
        self.modal.is_some()
    }

    /// Whether a panel is open.
    pub fn has_panel(&self, slot: PanelSlot) -> bool {
        match slot {
            PanelSlot::Side => self.side.is_some(),
            PanelSlot::Bottom => self.bottom.is_some(),
        }
    }

    /// The open views: (side panel, bottom panel, modal).
    pub(crate) fn views(&self) -> OpenViews<'_> {
        (
            self.side.as_deref(),
            self.bottom.as_deref(),
            self.modal.as_deref(),
        )
    }

    /// Show a notification for a few seconds (`[ui] notify_timeout_ms`).
    pub fn notify(&mut self, level: Level, text: impl Into<String>) {
        let text = text.into();
        let expires = (self.clock)() + Duration::from_millis(self.ui.notify_timeout_ms);
        self.history.push((level, text.clone()));
        self.notifications.push(Notification {
            level,
            text,
            expires,
        });
    }

    /// Notifications currently on screen, oldest first.
    pub fn notifications(&self) -> &[Notification] {
        &self.notifications
    }

    /// Messages and notifications so far, oldest first.
    pub fn history(&self) -> &[(Level, String)] {
        &self.history
    }

    /// When something time-based happens next (a notification expires or a pending mapping
    /// times out). The run loop should call [`Editor::tick`] then.
    pub fn next_deadline(&self) -> Option<Instant> {
        let mapping = self.pending_since.map(|t| t + self.key_timeout);
        let note = self.notifications.iter().map(|n| n.expires).min();
        // While jobs run, check for their messages often.
        let jobs = (self.jobs_running > 0).then(|| (self.clock)() + Duration::from_millis(100));
        mapping.into_iter().chain(note).chain(jobs).min()
    }

    /// Run `work` on a background thread (e.g. building a plugin). While it runs, a box
    /// shows `title`, the last progress `work` reported, and the time so far (see
    /// [`Editor::job_status`]). `work` returns what to do on the editor's thread when it is
    /// done. Results arrive in [`Editor::tick`].
    pub fn spawn_job(
        &mut self,
        title: &str,
        work: impl FnOnce(&dyn Fn(&str)) -> JobDone + Send + 'static,
    ) {
        self.history.push((Level::Info, format!("{title}…")));
        let id = self.start_job(work);
        self.job_status.push(ui::JobStatus {
            id,
            title: title.to_string(),
            detail: None,
            started: (self.clock)(),
        });
    }

    /// Like [`Editor::spawn_job`], but nothing is shown while it runs; progress only goes
    /// to the log.
    pub fn spawn_quiet_job(
        &mut self,
        work: impl FnOnce(&dyn Fn(&str)) -> JobDone + Send + 'static,
    ) {
        self.start_job(work);
    }

    /// Start `work` on a thread; returns its job id.
    fn start_job(&mut self, work: impl FnOnce(&dyn Fn(&str)) -> JobDone + Send + 'static) -> u64 {
        let id = self.next_job_id;
        self.next_job_id += 1;
        let tx = self.jobs_tx.clone();
        self.jobs_running += 1;
        std::thread::spawn(move || {
            let progress_tx = tx.clone();
            let progress = |text: &str| {
                let _ = progress_tx.send(JobMsg::Progress(id, text.to_string()));
            };
            let done = work(&progress);
            let _ = tx.send(JobMsg::Done(id, done));
        });
        id
    }

    /// The current time (from [`Services::clock`]).
    pub(crate) fn now(&self) -> Instant {
        (self.clock)()
    }

    /// Jobs shown on screen while they run, oldest first.
    pub fn job_status(&self) -> &[ui::JobStatus] {
        &self.job_status
    }

    /// Where vox updates come from, if set (see [`crate::update`]).
    pub fn update_source(&self) -> Option<&crate::update::UpdateSource> {
        self.update_source.as_ref()
    }

    /// Set where vox updates come from (the binary does this).
    pub fn set_update_source(&mut self, source: crate::update::UpdateSource) {
        self.update_source = Some(source);
    }

    /// The vox version `:update install` installed since start, if any; it runs once vox
    /// is restarted.
    pub fn update_installed(&self) -> Option<crate::plugin::Version> {
        self.update_installed
    }

    /// Record that vox `version` was installed (done by `:update install`).
    pub fn set_update_installed(&mut self, version: crate::plugin::Version) {
        self.update_installed = Some(version);
    }

    /// Number of background jobs still running.
    pub fn jobs_running(&self) -> usize {
        self.jobs_running
    }

    /// Set the parser and formatter for plugin and lock files.
    pub fn set_file_format(
        &mut self,
        parse: crate::config::ValueParser,
        format: crate::config::ValueFormatter,
    ) {
        self.file_format = Some((parse, format));
    }

    /// The parser and formatter for plugin and lock files, if set.
    pub fn file_format(
        &self,
    ) -> Option<(crate::config::ValueParser, crate::config::ValueFormatter)> {
        self.file_format
    }

    /// Handle time passing: finish background jobs, remove expired notifications, time
    /// out pending mappings.
    pub fn tick(&mut self) {
        while let Ok(msg) = self.jobs_rx.try_recv() {
            match msg {
                JobMsg::Progress(id, text) => {
                    self.log(Level::Info, "job", &text);
                    if let Some(j) = self.job_status.iter_mut().find(|j| j.id == id) {
                        j.detail = Some(text);
                    }
                }
                JobMsg::Done(id, done) => {
                    self.jobs_running = self.jobs_running.saturating_sub(1);
                    self.job_status.retain(|j| j.id != id);
                    done(self);
                    self.refresh_views();
                }
            }
        }
        let now = (self.clock)();
        self.notifications.retain(|n| n.expires > now);
        if self
            .pending_since
            .is_some_and(|t| now >= t + self.key_timeout)
        {
            self.pending_since = None;
            self.timeout();
        }
    }

    /// The UI settings in use.
    pub fn ui_config(&self) -> &UiConfig {
        &self.ui
    }

    /// Call once when the editor starts: sends [`Event::BufOpen`] for the first buffer.
    pub fn start(&mut self) {
        let path = self.doc.path().map(|p| p.display().to_string());
        self.emit(&Event::BufOpen { path });
    }

    /// Call `handler` for every event of `kind`.
    pub fn subscribe(&mut self, kind: EventKind, handler: impl Fn(&mut Editor, &Event) + 'static) {
        let owner = self.owner.clone();
        self.subscribers
            .push((kind, owner, std::rc::Rc::new(handler)));
    }

    /// Run `f` with `owner` as the owner of everything it registers (commands, mappings,
    /// subscriptions), so [`Editor::remove_owned`] can remove exactly those later.
    pub fn with_owner<R>(&mut self, owner: &str, f: impl FnOnce(&mut Self) -> R) -> R {
        let prev = std::mem::replace(&mut self.owner, owner.to_string());
        let prev_cmd = std::mem::replace(&mut self.commands.owner, owner.to_string());
        let prev_map = std::mem::replace(&mut self.keymap.owner, owner.to_string());
        let r = f(self);
        self.owner = prev;
        self.commands.owner = prev_cmd;
        self.keymap.owner = prev_map;
        r
    }

    /// Remove every command, mapping, and subscription added by `owner`. Things they hid
    /// (e.g. a built-in command a plugin replaced) become active again.
    pub fn remove_owned(&mut self, owner: &str) {
        self.commands.remove_owned(owner);
        self.keymap.remove_owned(owner);
        self.subscribers.retain(|(_, o, _)| o != owner);
    }

    /// Add a log entry. `source` is e.g. `editor`, `config`, or `plugin:NAME`.
    pub fn log(&mut self, level: Level, source: &str, message: impl AsRef<str>) {
        let now = (self.clock)();
        self.log.push(now, level, source, message.as_ref());
        self.refresh_log_views();
    }

    /// The log.
    pub fn log_entries(&self) -> &Log {
        &self.log
    }

    /// Clear the log.
    pub(crate) fn clear_log(&mut self) {
        self.log.clear();
        self.refresh_log_views();
    }

    /// Update open log views after the log or their filters changed.
    pub(crate) fn refresh_log_views(&mut self) {
        let log = &self.log;
        for view in [&mut self.side, &mut self.bottom, &mut self.modal]
            .into_iter()
            .flatten()
        {
            if let Some(lv) = view.as_log_view() {
                lv.refresh(log);
            }
        }
    }

    /// Set the active scope and the user's global scope (if known). Only informs `:scope`,
    /// the status line, and project set-up; loading config and plugins is done by the
    /// binary from the scope's paths.
    pub fn set_scope(&mut self, scope: Scope, global: Option<Scope>) {
        self.log(
            Level::Info,
            "editor",
            format!(
                "scope {} ({})",
                scope.kind.name(),
                scope
                    .dir
                    .as_ref()
                    .map_or("no folder".into(), |d| d.display().to_string())
            ),
        );
        self.scope = scope;
        self.global_scope = global;
    }

    /// The active scope.
    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// The user's global scope, if known (even when another scope is active).
    pub fn global_scope(&self) -> Option<&Scope> {
        self.global_scope.as_ref()
    }

    /// The config last applied.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Set the function that writes settings changes into the config file.
    pub fn set_config_writer(&mut self, writer: crate::settings::ConfigWriter) {
        self.config_writer = Some(writer);
    }

    /// Record the settings plugin `name` declares (empty: none); used by `:settings`.
    pub fn set_plugin_settings(&mut self, name: &str, specs: Vec<crate::plugin::SettingSpec>) {
        if specs.is_empty() {
            self.plugin_settings.remove(name);
        } else {
            self.plugin_settings.insert(name.to_string(), specs);
        }
    }

    /// Settings declared by each plugin, by name.
    pub fn plugin_settings(
        &self,
    ) -> &std::collections::BTreeMap<String, Vec<crate::plugin::SettingSpec>> {
        &self.plugin_settings
    }

    /// Apply `edits` to the config: check them, write them to the active scope's config file
    /// (keeping comments), and apply the result. Returns whether it was saved to a file (not
    /// in vanilla mode, or without a writer). Invalid edits are rejected before writing.
    pub fn save_config_edits(
        &mut self,
        edits: &[crate::settings::ConfigEdit],
    ) -> Result<bool, String> {
        let mut value = self.config.to_value();
        crate::settings::apply_edits(&mut value, edits);
        let (cfg, errors) = Config::from_value(&value);
        if let Some(e) = errors.first() {
            return Err(format!("E900: not saved: {e}"));
        }
        let saved = match (self.scope.config_path(), self.config_writer) {
            (Some(path), Some(write)) => {
                write(&path, edits)
                    .map_err(|e| format!("E900: could not save {}: {e}", path.display()))?;
                self.log(
                    Level::Info,
                    "config",
                    format!("saved {} change(s) to {}", edits.len(), path.display()),
                );
                true
            }
            _ => false,
        };
        self.apply_config(&cfg);
        Ok(saved)
    }

    /// Title of the open modal, if any.
    pub fn modal_title(&self) -> Option<String> {
        self.modal.as_ref().map(|m| m.title())
    }

    /// The user's `[plugins.NAME]` values (settings and `enabled`), if any.
    pub fn plugin_config(
        &self,
        name: &str,
    ) -> Option<&std::collections::BTreeMap<String, crate::config::Value>> {
        self.plugin_config.get(name)
    }

    /// Whether developer mode is on.
    pub fn developer_mode(&self) -> bool {
        self.developer_mode
    }

    /// Turn developer mode on or off. On: adds `<leader>dl` (log), `<leader>dr` (reload
    /// plugins), `<leader>dc` (reload config).
    pub fn set_developer_mode(&mut self, on: bool) {
        self.remove_owned(owner::DEVMODE);
        self.developer_mode = on;
        if on {
            self.with_owner(owner::DEVMODE, |ed| {
                ed.map_key(MapMode::Normal, "<leader>dl", ":log");
                ed.map_key(MapMode::Normal, "<leader>dr", ":reload plugins");
                ed.map_key(MapMode::Normal, "<leader>dc", ":reload config");
            });
        }
        self.log(
            Level::Info,
            "editor",
            format!("developer mode {}", if on { "on" } else { "off" }),
        );
    }

    /// Send `event` to its subscribers, in subscription order.
    fn emit(&mut self, event: &Event) {
        let handlers: Vec<EventHandler> = self
            .subscribers
            .iter()
            .filter(|(k, _, _)| *k == event.kind())
            .map(|(_, _, h)| h.clone())
            .collect();
        for h in handlers {
            h(self, event);
        }
    }

    /// Run `f`, then (if this is the outermost call) send `ModeChanged` / `TextChanged`
    /// for what `f` changed.
    fn with_change_events(&mut self, f: impl FnOnce(&mut Self)) {
        let (mode, rev) = (self.mode_kind(), self.doc.revision());
        let msg = self.message.clone();
        self.event_depth += 1;
        f(self);
        self.event_depth -= 1;
        if self.event_depth > 0 {
            return;
        }
        if self.message != msg
            && let Some(m) = &self.message
        {
            // `E123: ...` messages are errors, like Vim's.
            let is_error = m.starts_with('E') && m[1..].starts_with(|c: char| c.is_ascii_digit());
            let level = if is_error { Level::Error } else { Level::Info };
            self.history.push((level, m.clone()));
        }
        if self.doc.revision() != rev {
            self.emit(&Event::TextChanged);
        }
        let now = self.mode_kind();
        if now != mode {
            self.emit(&Event::ModeChanged { mode: now });
        }
    }

    /// The current mode without its data.
    pub fn mode_kind(&self) -> ModeKind {
        match self.mode {
            Mode::Normal => ModeKind::Normal,
            Mode::Insert => ModeKind::Insert,
            Mode::Visual { .. } => ModeKind::Visual,
            Mode::Command(_) => ModeKind::Command,
            Mode::Search(_) => ModeKind::Search,
        }
    }

    /// Insert `text` at `at` (clamped). Returns the position after the inserted text.
    /// Edits made by one command are undone together.
    pub fn insert_text(&mut self, at: Position, text: &str) -> Position {
        self.doc.insert(at, text)
    }

    /// Delete the text in `[from, to)` (clamped).
    pub fn delete_text(&mut self, from: Position, to: Position) {
        self.doc.delete(from, to);
    }

    /// Whether typed keys are waiting to see if they complete a mapping. The run loop
    /// should call [`Editor::timeout`] if no key arrives within [`Editor::key_timeout`].
    pub fn has_pending_keys(&self) -> bool {
        !self.map_pending.is_empty()
    }

    /// How long to wait for the rest of a mapping.
    pub fn key_timeout(&self) -> Duration {
        self.key_timeout
    }

    /// No key arrived in time: use the mapping typed so far, or treat the keys as unmapped.
    pub fn timeout(&mut self) {
        self.with_change_events(|ed| ed.resolve_mappings(true));
        self.follow_cursor();
    }

    /// Mapping mode for the current editor mode; prompts have no mappings.
    fn map_mode(&self) -> Option<MapMode> {
        match self.mode {
            Mode::Normal => Some(MapMode::Normal),
            Mode::Insert => Some(MapMode::Insert),
            Mode::Visual { .. } => Some(MapMode::Visual),
            Mode::Command(_) | Mode::Search(_) => None,
        }
    }

    /// Match pending keys against mappings. Runs a mapping when one matches; sends keys that
    /// cannot start a mapping to the mode handlers. `timed_out` stops waiting for longer
    /// mappings.
    fn resolve_mappings(&mut self, timed_out: bool) {
        while !self.map_pending.is_empty() {
            let lookup = match self.map_mode() {
                Some(m) => self.keymap.lookup(m, &self.map_pending),
                None => Lookup::None,
            };
            match lookup {
                Lookup::Prefix | Lookup::ExactAndPrefix(_) if !timed_out => return,
                Lookup::Exact(b) | Lookup::ExactAndPrefix(b) => {
                    let b = b.clone();
                    self.map_pending.clear();
                    self.run_binding(b);
                }
                Lookup::None | Lookup::Prefix => {
                    // The first key starts no mapping: handle it as typed, retry the rest.
                    let key = self.map_pending.remove(0);
                    self.dispatch(key);
                }
            }
        }
    }

    /// Run a mapping's binding: feed its keys (not re-mapped) or run its command.
    fn run_binding(&mut self, b: Binding) {
        match b {
            Binding::Keys(keys) => keys.into_iter().for_each(|k| self.dispatch(k)),
            Binding::Command(line) => self.run_command(&line),
        }
    }

    /// Send one key to the handler of the current mode, without mappings.
    fn dispatch(&mut self, key: Key) {
        match self.mode {
            Mode::Normal | Mode::Visual { .. } => self.handle_normal(key),
            Mode::Insert => self.handle_insert(key),
            Mode::Command(_) | Mode::Search(_) => self.handle_prompt(key),
        }
    }

    /// Index of the last line.
    fn last_line(&self) -> usize {
        self.doc.line_count() - 1
    }

    /// Highest column the cursor may occupy on `line` in the current mode.
    fn max_col(&self, line: usize) -> usize {
        let len = self.doc.line_len(line);
        match self.mode {
            Mode::Insert => len,
            _ => len.saturating_sub(1),
        }
    }

    /// Move the cursor to `pos` (clamped) and remember its column for vertical moves.
    pub fn set_cursor(&mut self, pos: Position) {
        let line = pos.line.min(self.last_line());
        self.cursor = Position::new(line, pos.col.min(self.max_col(line)));
        self.want_col = self.cursor.col;
    }

    /// Move `delta` lines, keeping the wanted column.
    fn move_lines(&mut self, delta: isize) {
        let line = self
            .cursor
            .line
            .saturating_add_signed(delta)
            .min(self.last_line());
        self.cursor = Position::new(line, self.want_col.min(self.max_col(line)));
    }

    /// Switch to insert mode with the cursor at `pos`.
    fn enter_insert(&mut self, pos: Position) {
        self.mode = Mode::Insert;
        self.set_cursor(pos);
    }

    /// Normal/visual mode: collect keys until they form a complete command, then run it.
    fn handle_normal(&mut self, key: Key) {
        if key.code == KeyCode::Esc {
            self.pending.clear();
            if matches!(self.mode, Mode::Visual { .. }) {
                self.mode = Mode::Normal;
            }
            return;
        }
        self.pending.push(key);
        let visual = matches!(self.mode, Mode::Visual { .. });
        match normal::parse(&self.pending, visual) {
            Parse::Incomplete => {}
            Parse::Invalid => self.pending.clear(),
            Parse::Done(cmd) => {
                self.pending.clear();
                self.exec(cmd);
            }
        }
    }

    /// Run one parsed normal/visual mode command (motion, operator, insert, paste, undo, ...).
    fn exec(&mut self, cmd: Cmd) {
        if let Some(r) = cmd.register
            && !register::is_valid(r)
        {
            self.message = Some(format!("E354: Invalid register name: '{r}'"));
            return;
        }
        let n = cmd.count.unwrap_or(1);
        let c = self.cursor;
        match cmd.action {
            Action::Move(Motion::Up) => self.move_lines(-(n as isize)),
            Action::Move(Motion::Down) => self.move_lines(n as isize),
            Action::Move(m) => {
                if let Some((pos, _)) = self.motion_target(m, cmd.count, false) {
                    self.set_cursor(pos);
                    if m == Motion::LineEnd {
                        self.want_col = usize::MAX;
                    }
                }
            }
            Action::Operate(op, target) => self.operate(op, target, cmd.count, cmd.register),
            Action::Insert(at) => {
                let pos = match at {
                    InsertAt::Before => c,
                    InsertAt::After => {
                        Position::new(c.line, (c.col + 1).min(self.doc.line_len(c.line)))
                    }
                    InsertAt::LineStart => motion::first_non_blank(&self.doc, c.line),
                    InsertAt::LineEnd => Position::new(c.line, self.doc.line_len(c.line)),
                    InsertAt::Below => {
                        let eol = Position::new(c.line, self.doc.line_len(c.line));
                        self.doc.insert(eol, self.doc.line_ending())
                    }
                    InsertAt::Above => {
                        self.doc
                            .insert(Position::new(c.line, 0), self.doc.line_ending());
                        Position::new(c.line, 0)
                    }
                };
                self.enter_insert(pos);
            }
            Action::Paste { before } => self.paste(before, n, cmd.register),
            Action::Undo => (0..n).for_each(|_| self.undo()),
            Action::Redo => (0..n).for_each(|_| self.redo()),
            Action::Scroll(s) => {
                let (_, height) = self.text_area();
                let half = (height / 2).max(1) as isize;
                let delta = match s {
                    Scroll::HalfDown => half,
                    Scroll::HalfUp => -half,
                    Scroll::PageDown => height as isize,
                    Scroll::PageUp => -(height as isize),
                };
                self.view.scroll_by(delta, self.doc.line_count());
                self.move_lines(delta);
            }
            Action::Visual { line } => match self.mode {
                Mode::Visual { line: cur } if cur == line => self.mode = Mode::Normal,
                Mode::Visual { .. } => self.mode = Mode::Visual { line },
                _ => {
                    self.anchor = c;
                    self.mode = Mode::Visual { line };
                }
            },
            Action::CommandLine => {
                self.message = None;
                self.mode = Mode::Command(String::new());
            }
            Action::Search => {
                self.message = None;
                self.mode = Mode::Search(String::new());
            }
            Action::Window(c) => self.window_cmd(c),
        }
    }

    /// Where motion `m` (repeated `count` times) leads from the cursor, and its kind.
    /// `for_op` enables operator-only rules (`dw` stops at end of line).
    fn motion_target(
        &mut self,
        m: Motion,
        count: Option<usize>,
        for_op: bool,
    ) -> Option<(Position, Kind)> {
        let c = self.cursor;
        let n = count.unwrap_or(1);
        let doc = &self.doc;
        let repeat = |f: fn(&Document, Position) -> Position| (0..n).fold(c, |p, _| f(doc, p));
        let line_at = |l: usize| l.min(doc.line_count() - 1);
        Some(match m {
            Motion::Left => (
                Position::new(c.line, c.col.saturating_sub(n)),
                Kind::Exclusive,
            ),
            Motion::Right => (
                Position::new(c.line, (c.col + n).min(doc.line_len(c.line))),
                Kind::Exclusive,
            ),
            Motion::Up => (
                Position::new(c.line.saturating_sub(n), self.want_col),
                Kind::Linewise,
            ),
            Motion::Down => (
                Position::new(line_at(c.line + n), self.want_col),
                Kind::Linewise,
            ),
            Motion::WordForward => {
                let mut p = c;
                for step in 0..n {
                    let next = match motion::next_word_start(doc, p) {
                        Some(next) => next,
                        // No further word: an operator reaches the end of the document.
                        None if for_op => {
                            let last = doc.line_count() - 1;
                            p = Position::new(last, doc.line_len(last));
                            break;
                        }
                        None => motion::word_forward(doc, p),
                    };
                    if for_op && step == n - 1 && next.line > p.line {
                        // `dw` on the last word of a line stops at the line end.
                        p = Position::new(p.line, doc.line_len(p.line));
                        break;
                    }
                    p = next;
                }
                (p, Kind::Exclusive)
            }
            Motion::WordBackward => (repeat(motion::word_backward), Kind::Exclusive),
            Motion::WordEnd => (repeat(motion::word_end), Kind::Inclusive),
            Motion::LineStart => (motion::line_start(c), Kind::Exclusive),
            Motion::FirstNonBlank => (motion::first_non_blank(doc, c.line), Kind::Exclusive),
            Motion::LineEnd => {
                let line = line_at(c.line + n - 1);
                (
                    motion::line_end(doc, Position::new(line, 0)),
                    Kind::Inclusive,
                )
            }
            Motion::FileStart => (
                motion::first_non_blank(doc, count.map_or(0, |l| line_at(l.saturating_sub(1)))),
                Kind::Linewise,
            ),
            Motion::FileEnd => (
                motion::first_non_blank(
                    doc,
                    count.map_or(doc.line_count() - 1, |l| line_at(l.saturating_sub(1))),
                ),
                Kind::Linewise,
            ),
            Motion::SearchNext | Motion::SearchPrev => {
                let pos = self.search_target(m == Motion::SearchNext, n)?;
                (pos, Kind::Exclusive)
            }
        })
    }

    /// Repeat the last search `n` times. Sets messages for wrap / not found.
    fn search_target(&mut self, forward: bool, n: usize) -> Option<Position> {
        let Some(pattern) = self.last_search.clone() else {
            self.message = Some("E35: No previous regular expression".into());
            return None;
        };
        let mut pos = self.cursor;
        let mut wrapped = false;
        for _ in 0..n {
            match search::find(&self.doc, pos, &pattern, forward) {
                Some(m) => {
                    pos = m.pos;
                    wrapped |= m.wrapped;
                }
                None => {
                    self.message = Some(format!("E486: Pattern not found: {pattern}"));
                    return None;
                }
            }
        }
        self.message = if wrapped {
            Some(
                if forward {
                    "search hit BOTTOM, continuing at TOP"
                } else {
                    "search hit TOP, continuing at BOTTOM"
                }
                .into(),
            )
        } else {
            Some(format!("/{pattern}"))
        };
        Some(pos)
    }

    /// Work out the text range an operator acts on (from a motion, `dd`-style lines, or the visual selection), then apply it.
    fn operate(&mut self, op: Operator, target: Target, count: Option<usize>, reg: Option<char>) {
        let c = self.cursor;
        let n = count.unwrap_or(1);
        let range = match target {
            Target::Lines => Range {
                start: Position::new(c.line, 0),
                end: Position::new((c.line + n - 1).min(self.last_line()), 0),
                linewise: true,
            },
            Target::Selection => {
                let Some((a, b, line)) = self.selection() else {
                    return;
                };
                self.mode = Mode::Normal;
                Range {
                    start: a,
                    end: Position::new(b.line, b.col + 1),
                    linewise: line,
                }
            }
            Target::Motion(Motion::WordForward)
                if op == Operator::Change
                    && self
                        .doc
                        .line(c.line)
                        .and_then(|l| l.chars().nth(c.col))
                        .is_some_and(|ch| !ch.is_whitespace()) =>
            {
                // `cw` on a word changes to the end of the word, like `ce`.
                let first = motion::current_word_end(&self.doc, c);
                let end = (1..n).fold(first, |p, _| motion::word_end(&self.doc, p));
                Range {
                    start: c,
                    end: Position::new(end.line, end.col + 1),
                    linewise: false,
                }
            }
            Target::Motion(m) => {
                let Some((t, kind)) = self.motion_target(m, count, true) else {
                    return;
                };
                let (a, b) = order(c, t);
                match kind {
                    Kind::Linewise => Range {
                        start: Position::new(a.line, 0),
                        end: Position::new(b.line, 0),
                        linewise: true,
                    },
                    Kind::Inclusive => Range {
                        start: a,
                        end: Position::new(b.line, b.col + 1),
                        linewise: false,
                    },
                    Kind::Exclusive => Range {
                        start: a,
                        end: b,
                        linewise: false,
                    },
                }
            }
        };
        self.apply(op, range, reg);
    }

    /// Apply operator `op` to range `r`: save the text into the register, then delete, change, or yank.
    fn apply(&mut self, op: Operator, r: Range, reg: Option<char>) {
        let (a, b) = (r.start.line, r.end.line);
        let text = if r.linewise {
            let body = self
                .doc
                .slice(Position::new(a, 0), Position::new(b, self.doc.line_len(b)));
            format!("{body}\n")
        } else {
            self.doc.slice(r.start, r.end)
        };
        let reg_value = Reg {
            text: text.replace("\r\n", "\n"),
            linewise: r.linewise,
        };
        if let Err(e) = self.registers.set(reg, reg_value) {
            self.message = Some(format!("Clipboard error: {e}"));
        }
        match op {
            Operator::Yank => {
                if r.linewise {
                    self.set_cursor(Position::new(a, self.cursor.col));
                } else {
                    self.set_cursor(r.start);
                }
            }
            Operator::Delete => {
                if r.linewise {
                    self.delete_lines(a, b);
                    self.set_cursor(motion::first_non_blank(&self.doc, a.min(self.last_line())));
                } else {
                    self.doc.delete(r.start, r.end);
                    self.set_cursor(r.start);
                }
                self.doc.commit();
            }
            Operator::Change => {
                if r.linewise {
                    let end = Position::new(b, self.doc.line_len(b));
                    self.doc.delete(Position::new(a, 0), end);
                    self.enter_insert(Position::new(a, 0));
                } else {
                    self.doc.delete(r.start, r.end);
                    self.enter_insert(r.start);
                }
            }
        }
    }

    /// Delete lines `a..=b` including their line breaks.
    fn delete_lines(&mut self, a: usize, b: usize) {
        let len = |l| self.doc.line_len(l);
        if b < self.last_line() {
            self.doc
                .delete(Position::new(a, 0), Position::new(b + 1, 0));
        } else if a > 0 {
            let (from, to) = (Position::new(a - 1, len(a - 1)), Position::new(b, len(b)));
            self.doc.delete(from, to);
        } else {
            let to = Position::new(b, len(b));
            self.doc.delete(Position::new(0, 0), to);
        }
    }

    /// `p` / `P`: insert register text `n` times after or before the cursor (below or above for whole lines).
    fn paste(&mut self, before: bool, n: usize, reg: Option<char>) {
        let reg = match self.registers.get(reg) {
            Ok(r) => r,
            Err(e) => {
                self.message = Some(e);
                return;
            }
        };
        let le = self.doc.line_ending();
        let content = reg.text.repeat(n);
        let c = self.cursor;
        if reg.linewise {
            let body = content
                .strip_suffix('\n')
                .unwrap_or(&content)
                .replace('\n', le);
            let line = if before {
                self.doc
                    .insert(Position::new(c.line, 0), &format!("{body}{le}"));
                c.line
            } else {
                let eol = Position::new(c.line, self.doc.line_len(c.line));
                self.doc.insert(eol, &format!("{le}{body}"));
                c.line + 1
            };
            self.set_cursor(motion::first_non_blank(&self.doc, line));
        } else {
            let body = content.replace('\n', le);
            let at = if before || self.doc.line_len(c.line) == 0 {
                c
            } else {
                Position::new(c.line, c.col + 1)
            };
            let end = self.doc.insert(at, &body);
            if body.contains('\n') {
                self.set_cursor(at);
            } else {
                self.set_cursor(Position::new(end.line, end.col.saturating_sub(1)));
            }
        }
        self.doc.commit();
    }

    /// Insert mode: type text, new lines, backspace, arrows; `Esc` returns to normal mode.
    fn handle_insert(&mut self, key: Key) {
        let c = self.cursor;
        match key.code {
            KeyCode::Esc => {
                self.doc.commit();
                self.mode = Mode::Normal;
                self.set_cursor(Position::new(c.line, c.col.saturating_sub(1)));
            }
            KeyCode::Char(ch) if !key.mods.ctrl => {
                let end = self.doc.insert(c, ch.encode_utf8(&mut [0; 4]));
                self.set_cursor(end);
            }
            KeyCode::Tab => {
                let end = self.doc.insert(c, "\t");
                self.set_cursor(end);
            }
            KeyCode::Enter => {
                let end = self.doc.insert(c, self.doc.line_ending());
                self.set_cursor(end);
            }
            KeyCode::Backspace => {
                if c.col > 0 {
                    let prev = Position::new(c.line, c.col - 1);
                    self.doc.delete(prev, c);
                    self.set_cursor(prev);
                } else if c.line > 0 {
                    let prev = Position::new(c.line - 1, self.doc.line_len(c.line - 1));
                    self.doc.delete(prev, c);
                    self.set_cursor(prev);
                }
            }
            KeyCode::Left => self.set_cursor(Position::new(c.line, c.col.saturating_sub(1))),
            KeyCode::Right => self.set_cursor(Position::new(c.line, c.col + 1)),
            KeyCode::Up => self.move_lines(-1),
            KeyCode::Down => self.move_lines(1),
            _ => {}
        }
    }

    /// `u`: undo one step and move the cursor to the change.
    fn undo(&mut self) {
        match self.doc.undo() {
            Some(pos) => self.set_cursor(pos),
            None => self.message = Some("Already at oldest change".into()),
        }
    }

    /// `Ctrl-r`: redo one step and move the cursor to the change.
    fn redo(&mut self) {
        match self.doc.redo() {
            Some(pos) => self.set_cursor(pos),
            None => self.message = Some("Already at newest change".into()),
        }
    }

    /// Key handling for the `:` and `/` prompts.
    fn handle_prompt(&mut self, key: Key) {
        let (Mode::Command(input) | Mode::Search(input)) = &mut self.mode else {
            return;
        };
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                if input.pop().is_none() {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Enter => {
                let input = std::mem::take(input);
                let mode = std::mem::replace(&mut self.mode, Mode::Normal);
                match mode {
                    Mode::Search(_) => self.search(input),
                    _ => self.run_command(&input),
                }
            }
            KeyCode::Char(c) => input.push(c),
            _ => {}
        }
    }

    /// Run a `/` search for `pattern` (empty = repeat the last pattern).
    fn search(&mut self, pattern: String) {
        if !pattern.is_empty() {
            self.last_search = Some(pattern);
        }
        if let Some(pos) = self.search_target(true, 1) {
            self.set_cursor(pos);
        }
    }

    /// Run a command line such as `w`, `e! notes.txt`, or a user/plugin command.
    /// Errors are shown as the message; data the command replied with (see
    /// [`Editor::reply`]) is shown in the message line or a modal.
    pub fn run_command(&mut self, line: &str) {
        self.with_change_events(|ed| match ed.exec_command(line) {
            Ok(Some(v)) => ed.show_reply(line, &v),
            Ok(None) => {}
            Err(e) => ed.message = Some(e),
        });
        self.refresh_if_stale();
    }

    /// Run a command line and return its outcome instead of showing it: the data it
    /// replied with (see [`Editor::reply`]), or its error. Used by `vox run` and tests.
    pub fn execute(&mut self, line: &str) -> Result<Option<crate::config::Value>, String> {
        let mut out = Ok(None);
        self.with_change_events(|ed| out = ed.exec_command(line));
        self.refresh_if_stale();
        out
    }

    /// Return `value` as the result of the running command. [`Editor::execute`] hands it to
    /// the caller; [`Editor::run_command`] shows it.
    pub fn reply(&mut self, value: crate::config::Value) {
        self.reply = Some(value);
    }

    /// Whether a person is at the keyboard (the default). When not (`vox run`), commands
    /// must not ask: a command that opens a view fails, and commands that would ask for a
    /// missing argument or a confirmation fail instead.
    pub fn interactive(&self) -> bool {
        self.interactive
    }

    /// Set whether a person is at the keyboard (see [`Editor::interactive`]).
    pub fn set_interactive(&mut self, on: bool) {
        self.interactive = on;
    }

    /// Refresh open views once the outermost key or command is done.
    fn refresh_if_stale(&mut self) {
        if self.views_stale && self.event_depth == 0 {
            self.refresh_views();
        }
    }

    /// Show a command's reply: one line in the message line, more in a modal.
    fn show_reply(&mut self, line: &str, value: &crate::config::Value) {
        let text = value.to_text();
        if !text.contains('\n') {
            self.message = Some(text);
        } else if self.interactive {
            let lines = text
                .lines()
                .map(|l| (crate::surface::Style::default(), l.to_string()))
                .collect();
            self.open_modal(Box::new(ui::TextView::new(
                format!(":{}", line.trim()),
                lines,
            )));
        }
    }

    /// Find and run the command for `line`; see [`Editor::execute`].
    fn exec_command(&mut self, line: &str) -> Result<Option<crate::config::Value>, String> {
        // `:?plugin add` is short for `:help plugin add`.
        if let Some(topic) = line.trim_start().strip_prefix('?') {
            return self.exec_command(&format!("help {topic}"));
        }
        let (root, args) = command::parse_line(line);
        if root.is_empty() {
            return Ok(None);
        }
        // `:reload plugins x` runs the command "reload plugins" with the argument "x".
        let sub = args.arg.and_then(|a| {
            let (word, rest) = match a.split_once(char::is_whitespace) {
                Some((w, r)) => (w, Some(r.trim()).filter(|r| !r.is_empty())),
                None => (a, None),
            };
            // `:plugin remove! x`: the `!` may follow the subcommand.
            let (word, bang) = match word.strip_suffix('!') {
                Some(w) => (w, true),
                None => (word, false),
            };
            let name = format!("{root} {word}");
            self.commands
                .get(&name)
                .is_some()
                .then_some((name, rest, bang))
        });
        let (name, args) = match sub {
            Some((name, rest, bang)) => (
                name,
                command::Args {
                    arg: rest,
                    force: args.force || bang,
                },
            ),
            None => (root.to_string(), args),
        };
        let Some(cmd) = self.commands.get(&name) else {
            let subs: Vec<&str> = self
                .commands
                .subcommands_of(root)
                .map(|c| &c.name[root.len() + 1..])
                .collect();
            return Err(if subs.is_empty() {
                format!("E492: Not an editor command: {}", line.trim())
            } else {
                format!("E471: Use :{root} {}", subs.join("|"))
            });
        };
        let handler = cmd.handler.clone();
        let (cmd_name, cmd_owner) = (cmd.name.clone(), cmd.owner.clone());
        self.log(
            Level::Debug,
            "editor",
            format!(":{} ({cmd_owner})", line.trim()),
        );
        // Commands run from this one keep their own reply and refusal.
        let outer_reply = self.reply.take();
        let outer_refused = std::mem::take(&mut self.view_refused);
        let result = handler(self, args);
        let reply = std::mem::replace(&mut self.reply, outer_reply);
        let refused = std::mem::replace(&mut self.view_refused, outer_refused);
        // A command's edits are one undo step; keep the cursor valid after them.
        if self.mode != Mode::Insert {
            self.doc.commit();
        }
        self.set_cursor(self.cursor);
        self.views_stale = true;
        let result = match result {
            Ok(()) if refused => Err(format!(
                "E900: :{cmd_name} needs the editor UI (not available here)"
            )),
            r => r,
        };
        if let Err(e) = &result {
            self.log(Level::Error, &cmd_owner, format!(":{cmd_name}: {e}"));
        }
        result.map(|()| reply)
    }

    /// Let open views (panels, modal) update from editor state.
    pub fn refresh_views(&mut self) {
        self.views_stale = false;
        for slot in [Some(PanelSlot::Side), Some(PanelSlot::Bottom), None] {
            let taken = match slot {
                None => self.modal.take(),
                Some(PanelSlot::Side) => self.side.take(),
                Some(PanelSlot::Bottom) => self.bottom.take(),
            };
            if let Some(mut view) = taken {
                view.refresh(self);
                self.put_view(slot, view);
            }
        }
    }

    /// What the plugin host knows about each plugin (status, findings, settings).
    pub fn plugin_infos(&self) -> &[crate::plugin::PluginInfo] {
        &self.plugin_infos
    }

    /// Set by the plugin host after loading or reloading plugins.
    pub fn set_plugin_infos(&mut self, infos: Vec<crate::plugin::PluginInfo>) {
        self.plugin_infos = infos;
    }

    /// The command registry, to look up or list commands.
    pub fn commands(&self) -> &Commands {
        &self.commands
    }

    /// The command registry, to add commands.
    pub fn commands_mut(&mut self) -> &mut Commands {
        &mut self.commands
    }

    /// The user key mappings.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// The user key mappings, to change them.
    pub fn keymap_mut(&mut self) -> &mut Keymap {
        &mut self.keymap
    }

    /// Apply settings: leader, mapping timeout, and key mappings.
    /// Earlier config mappings are replaced, so this also reloads a config.
    pub fn apply_config(&mut self, cfg: &Config) {
        self.key_timeout = Duration::from_millis(cfg.timeout_ms);
        self.ui = cfg.ui;
        self.leader = cfg.leader.clone();
        self.config = cfg.clone();
        self.plugin_config = cfg.plugins.clone();
        self.default_mappings();
        self.remove_owned(owner::CONFIG);
        self.with_owner(owner::CONFIG, |ed| {
            for (mode, lhs, rhs) in &cfg.keymaps {
                ed.map_key(*mode, lhs, rhs);
            }
        });
        if cfg.developer_mode != self.developer_mode {
            self.set_developer_mode(cfg.developer_mode);
        }
    }

    /// vox's default mappings (owner `defaults`), made again with the current leader. Config
    /// mappings of the same keys win.
    pub(crate) fn default_mappings(&mut self) {
        self.remove_owned(owner::DEFAULTS);
        self.with_owner(owner::DEFAULTS, |ed| {
            ed.map_key(MapMode::Normal, "<leader>p", ":plugins");
        });
    }

    /// Map `lhs` to `rhs` in `mode`, both in Vim notation with `<leader>` allowed. `rhs`
    /// starting with `:` runs a command; an empty `rhs` disables the key.
    pub fn map_key(&mut self, mode: MapMode, lhs: &str, rhs: &str) {
        let keys = keymap::parse_lhs(lhs, &self.leader);
        let binding = keymap::parse_binding(rhs, &self.leader);
        self.keymap.set(mode, keys, binding);
    }

    /// Show `msg` on the bottom line.
    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = Some(msg.into());
    }

    /// Ask the editor to exit; the run loop stops after the current key.
    pub fn quit(&mut self) {
        self.quit = true;
    }

    /// Save, optionally to a new path. Sets the success message; returns bytes written.
    pub(crate) fn write(&mut self, path: Option<&str>) -> Result<usize, String> {
        if path.is_none() && self.doc.path().is_none() {
            return Err("E32: No file name".into());
        }
        let result = match path {
            Some(p) => self.doc.save_as(p),
            None => self.doc.save(),
        };
        let name = self
            .doc
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        match result {
            Ok(bytes) => {
                self.message = Some(format!(
                    "\"{name}\" {}L, {bytes}B written",
                    self.doc.line_count()
                ));
                self.emit(&Event::BufWrite { path: name });
                Ok(bytes)
            }
            Err(e) => Err(format!("E212: Can't open file for writing: {name}: {e}")),
        }
    }

    /// Open `path`, or reload the current file when `path` is `None`. Discards changes;
    /// callers check [`Document::is_modified`] first.
    pub(crate) fn edit(&mut self, path: Option<&str>) -> Result<(), String> {
        let path = match path.map(std::path::PathBuf::from) {
            Some(p) => p,
            None => match self.doc.path() {
                Some(p) => p.to_path_buf(),
                None => return Err("E32: No file name".into()),
            },
        };
        let doc = Document::open(&path, self.storage)
            .map_err(|e| format!("E484: Can't open file {}: {e}", path.display()))?;
        self.message = Some(format!("\"{}\" {}L", path.display(), doc.line_count()));
        self.doc = doc;
        self.view = Viewport::default();
        self.set_cursor(Position::default());
        self.emit(&Event::BufOpen {
            path: Some(path.display().to_string()),
        });
        Ok(())
    }
}

/// The open views: (side panel, bottom panel, modal).
pub(crate) type OpenViews<'a> = (
    Option<&'a dyn View>,
    Option<&'a dyn View>,
    Option<&'a dyn View>,
);

/// What a finished background job does on the editor's thread.
pub type JobDone = Box<dyn FnOnce(&mut Editor) + Send>;

/// A message from a background job.
enum JobMsg {
    /// Job id, progress text.
    Progress(u64, String),
    /// Job id, what to do on the editor's thread.
    Done(u64, JobDone),
}

/// The focus value of a panel slot.
fn focus_of(slot: PanelSlot) -> Focus {
    match slot {
        PanelSlot::Side => Focus::Side,
        PanelSlot::Bottom => Focus::Bottom,
    }
}

/// Return `a` and `b` in document order.
fn order(a: Position, b: Position) -> (Position, Position) {
    if b < a { (b, a) } else { (a, b) }
}
