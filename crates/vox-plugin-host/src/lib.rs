//! Runs plugins: WebAssembly components that follow the contract in `wit/vox.wit`.
//!
//! Uses the `wasmtime` library to run them and `wasmtime-wasi` to give them the minimal
//! system interface Rust's standard library expects (with no files, network, or
//! environment). Each call into a plugin has a time limit (about 1 s) and each plugin a memory
//! limit (64 MiB). A plugin that breaks a limit or crashes is disabled; the editor keeps
//! running.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use vox_core::Position;
use vox_editor::{Editor, Event, EventKind, MapMode, ModeKind};
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

/// Code generated from `wit/vox.wit`.
#[allow(missing_docs, clippy::all)]
mod bindings {
    wasmtime::component::bindgen!({
        world: "plugin",
        path: "../../wit",
    });
}

use bindings::Plugin;
use bindings::vox::plugin::types as wit;

/// How often the time-limit clock ticks.
const TICK: Duration = Duration::from_millis(10);
/// Time budget of one call into a plugin, in ticks (100 × 10 ms ≈ 1 s).
const CALL_BUDGET_TICKS: u64 = 100;
/// Memory limit per plugin.
const MEMORY_LIMIT: usize = 64 << 20;

/// Loads and runs plugins.
pub struct PluginHost {
    engine: Engine,
    linker: Linker<HostState>,
}

impl PluginHost {
    /// Create a host. Starts a background thread that drives call time limits.
    pub fn new() -> Result<Self, String> {
        let mut config = Config::new();
        config.epoch_interruption(true);
        let engine = Engine::new(&config).map_err(|e| e.to_string())?;
        let ticker = engine.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(TICK);
                ticker.increment_epoch();
            }
        });
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| e.to_string())?;
        Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(|e| e.to_string())?;
        Ok(Self { engine, linker })
    }

    /// Load the plugin at `path` into `editor`: compile it, call its `init`, and install the
    /// commands, mappings, and event subscriptions it asked for. Returns the plugin's name
    /// (the file name without `.wasm`).
    pub fn load(&self, editor: &mut Editor, path: &Path) -> Result<String, String> {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "plugin".into());
        let fail = |e: wasmtime::Error| format!("plugin {name}: {e}");
        let component = Component::from_file(&self.engine, path).map_err(fail)?;
        let mut store = Store::new(
            &self.engine,
            HostState {
                editor: EditorPtr(std::ptr::null_mut()),
                requests: Vec::new(),
                limits: StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT).build(),
                wasi: WasiCtx::builder().build(),
                table: ResourceTable::new(),
            },
        );
        store.limiter(|s| &mut s.limits);
        store.set_epoch_deadline(CALL_BUDGET_TICKS);
        let bindings = Plugin::instantiate(&mut store, &component, &self.linker).map_err(fail)?;
        let plugin = Rc::new(RefCell::new(Instance {
            name: name.clone(),
            store,
            bindings,
            disabled: false,
        }));
        call(&plugin, editor, |b, s| b.call_init(s))??;
        Ok(name)
    }

    /// Load every `*.wasm` file in `dir` (sorted by name). Returns one result per file.
    /// A missing directory yields no results.
    pub fn load_dir(&self, editor: &mut Editor, dir: &Path) -> Vec<Result<String, String>> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "wasm"))
            .collect();
        paths.sort();
        paths.iter().map(|p| self.load(editor, p)).collect()
    }
}

/// One loaded plugin.
struct Instance {
    name: String,
    store: Store<HostState>,
    bindings: Plugin,
    /// Set after a crash or broken limit; the plugin is not called again.
    disabled: bool,
}

/// Pointer to the editor, valid only while the editor is inside a call into the plugin.
struct EditorPtr(*mut Editor);

// SAFETY: the store (and so `HostState`) never leaves the editor's thread. `Send` is only
// required by the WASI trait bounds.
unsafe impl Send for EditorPtr {}

/// Registration requested by the plugin during a call; installed after the call returns,
/// when the host has the plugin's handle again.
enum Request {
    Command { name: String, description: String },
    Subscribe(EventKind),
}

/// Per-plugin state seen by the host functions.
struct HostState {
    editor: EditorPtr,
    requests: Vec<Request>,
    limits: StoreLimits,
    wasi: WasiCtx,
    table: ResourceTable,
}

impl HostState {
    /// The editor lent to the plugin for the current call.
    fn editor(&mut self) -> &mut Editor {
        assert!(
            !self.editor.0.is_null(),
            "plugin called the editor outside a call"
        );
        // SAFETY: `call` sets the pointer from a live `&mut Editor` for the duration of the
        // call and does not use that reference meanwhile; it clears it afterwards.
        unsafe { &mut *self.editor.0 }
    }
}

impl WasiView for HostState {
    /// WASI state (no files, network, or environment) required by `wasmtime-wasi`.
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// Call into `plugin` with access to `editor`, then install any registrations it requested.
/// Errors (crash, time or memory limit) disable the plugin.
fn call<R>(
    plugin: &Rc<RefCell<Instance>>,
    editor: &mut Editor,
    f: impl FnOnce(&Plugin, &mut Store<HostState>) -> wasmtime::Result<R>,
) -> Result<R, String> {
    let (result, requests, name) = {
        let mut guard = plugin
            .try_borrow_mut()
            .map_err(|_| "plugin is already running (it called itself)".to_string())?;
        let inst = &mut *guard;
        if inst.disabled {
            return Err(format!(
                "plugin {} is disabled after an earlier error",
                inst.name
            ));
        }
        inst.store.data_mut().editor = EditorPtr(editor);
        inst.store.set_epoch_deadline(CALL_BUDGET_TICKS);
        let result = f(&inst.bindings, &mut inst.store);
        inst.store.data_mut().editor = EditorPtr(std::ptr::null_mut());
        if result.is_err() {
            inst.disabled = true;
        }
        let requests = std::mem::take(&mut inst.store.data_mut().requests);
        (result, requests, inst.name.clone())
    };
    for req in requests {
        install(plugin, editor, req);
    }
    result.map_err(|e| describe(&name, &e))
}

/// A readable message for a failed call.
fn describe(name: &str, e: &wasmtime::Error) -> String {
    let reason = match e.downcast_ref::<wasmtime::Trap>() {
        Some(wasmtime::Trap::Interrupt) => "took too long".to_string(),
        Some(trap) => trap.to_string(),
        None => format!("{e:#}"),
    };
    format!("plugin {name} stopped: {reason} (disabled)")
}

/// Install one registration: a command that calls back into the plugin, or an event
/// subscription that forwards events to it.
fn install(plugin: &Rc<RefCell<Instance>>, editor: &mut Editor, req: Request) {
    match req {
        Request::Command { name, description } => {
            let p = plugin.clone();
            let cmd = name.clone();
            editor
                .commands_mut()
                .register(&name, &description, move |ed, args| {
                    let arg = args.arg.map(str::to_string);
                    call(&p, ed, |b, s| {
                        b.call_run_command(s, &cmd, arg.as_deref(), args.force)
                    })?
                });
        }
        Request::Subscribe(kind) => {
            let p = plugin.clone();
            editor.subscribe(kind, move |ed, event| {
                let ev = to_wit_event(event);
                if let Err(e) = call(&p, ed, |b, s| b.call_on_event(s, &ev)) {
                    ed.set_message(e);
                }
            });
        }
    }
}

/// Convert a core mode to the WIT `mode`.
fn to_wit_mode(m: ModeKind) -> wit::Mode {
    match m {
        ModeKind::Normal => wit::Mode::Normal,
        ModeKind::Insert => wit::Mode::Insert,
        ModeKind::Visual => wit::Mode::Visual,
        ModeKind::Command => wit::Mode::Command,
        ModeKind::Search => wit::Mode::Search,
    }
}

/// Convert a core event to the WIT `event`.
fn to_wit_event(e: &Event) -> wit::Event {
    match e {
        Event::BufOpen { path } => wit::Event::BufOpen(path.clone()),
        Event::BufWrite { path } => wit::Event::BufWrite(path.clone()),
        Event::TextChanged => wit::Event::TextChanged,
        Event::ModeChanged { mode } => wit::Event::ModeChanged(to_wit_mode(*mode)),
    }
}

/// WIT position → core position.
fn to_pos(p: wit::Position) -> Position {
    Position::new(p.line as usize, p.col as usize)
}

/// Core position → WIT position.
fn from_pos(p: Position) -> wit::Position {
    wit::Position {
        line: p.line as u32,
        col: p.col as u32,
    }
}

impl wit::Host for HostState {}

impl bindings::vox::plugin::editor::Host for HostState {
    /// WIT `register-command`: queue a new command; installed after the call.
    fn register_command(&mut self, name: String, description: String) {
        self.requests.push(Request::Command { name, description });
    }

    /// WIT `map-key`: add a key mapping now, using the user's leader key.
    fn map_key(&mut self, mode: wit::MapMode, keys: String, binding: String) {
        let mode = match mode {
            wit::MapMode::Normal => MapMode::Normal,
            wit::MapMode::Insert => MapMode::Insert,
            wit::MapMode::Visual => MapMode::Visual,
        };
        self.editor().map_key(mode, &keys, &binding);
    }

    /// WIT `subscribe`: queue an event subscription; installed after the call.
    fn subscribe(&mut self, kind: wit::EventKind) {
        let kind = match kind {
            wit::EventKind::BufOpen => EventKind::BufOpen,
            wit::EventKind::BufWrite => EventKind::BufWrite,
            wit::EventKind::TextChanged => EventKind::TextChanged,
            wit::EventKind::ModeChanged => EventKind::ModeChanged,
        };
        self.requests.push(Request::Subscribe(kind));
    }

    /// WIT `message`: show text on the bottom line.
    fn message(&mut self, text: String) {
        self.editor().set_message(text);
    }

    /// WIT `run-command`: run a command line in the editor.
    fn run_command(&mut self, line: String) {
        self.editor().run_command(&line);
    }

    /// WIT `file-path`: path of the current file.
    fn file_path(&mut self) -> Option<String> {
        self.editor()
            .document()
            .path()
            .map(|p| p.display().to_string())
    }

    /// WIT `line-count`: number of lines.
    fn line_count(&mut self) -> u32 {
        self.editor().document().line_count() as u32
    }

    /// WIT `line`: text of one line.
    fn line(&mut self, index: u32) -> Option<String> {
        self.editor().document().line(index as usize)
    }

    /// WIT `insert-text`: insert text through the core edit API.
    fn insert_text(&mut self, at: wit::Position, text: String) -> wit::Position {
        from_pos(self.editor().insert_text(to_pos(at), &text))
    }

    /// WIT `delete-text`: delete text through the core edit API.
    fn delete_text(&mut self, start: wit::Position, end: wit::Position) {
        self.editor().delete_text(to_pos(start), to_pos(end));
    }

    /// WIT `cursor`: cursor position.
    fn cursor(&mut self) -> wit::Position {
        from_pos(self.editor().cursor())
    }

    /// WIT `set-cursor`: move the cursor.
    fn set_cursor(&mut self, pos: wit::Position) {
        self.editor().set_cursor(to_pos(pos));
    }

    /// WIT `current-mode`: the editor's mode.
    fn current_mode(&mut self) -> wit::Mode {
        to_wit_mode(self.editor().mode_kind())
    }
}
