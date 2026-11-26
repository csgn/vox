//! Runs plugins: WebAssembly components that follow the contract in `wit/vox.wit`.
//!
//! Uses the `wasmtime` library to run them and `wasmtime-wasi` to give them the minimal
//! system interface Rust's standard library expects (with no files, network, or
//! environment). Each call into a plugin has a time limit (about 1 s) and each plugin a memory
//! limit (64 MiB). A plugin that breaks a limit or crashes is disabled; the editor keeps
//! running.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;
use std::rc::Weak;
use std::time::Duration;

use std::path::PathBuf;
use vox_core::Position;

use vox_editor::config::{Value, ValueParser};
use vox_editor::plugin::package::{self, AssessEnv, Package, PluginInfo};
use vox_editor::plugin::{API_VERSION, HealthItem, HealthStatus, Version, health};
use vox_editor::ui::TextView;
use vox_editor::{Color, Editor, Event, EventKind, Level, MapMode, ModeKind, Style, owner};
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
    /// Reads `plugin.toml` text into a [`Value`] (TOML parser from an adapter).
    parse: ValueParser,
    /// Every plugin seen so far (loaded, failed, or disabled), with its running instance.
    registry: RefCell<Vec<Entry>>,
}

/// A plugin known to the host.
struct Entry {
    info: PluginInfo,
    /// The running plugin; gone once its commands and handlers are removed.
    instance: Weak<RefCell<Instance>>,
}

/// This vox's version, for `vox = ">=..."` requirements.
fn vox_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("crate version is valid")
}

impl PluginHost {
    /// Create a host. `parse` reads `plugin.toml` files. Starts a background thread that
    /// drives call time limits.
    pub fn new(parse: ValueParser) -> Result<Self, String> {
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
        Ok(Self {
            engine,
            linker,
            parse,
            registry: RefCell::new(Vec::new()),
        })
    }

    /// Load the plugin at `path` (a package folder or a `.wasm` file): check its health,
    /// then compile it, call `init` and `health`, and install what it registered. Returns
    /// the plugin's name, or why it was not loaded.
    pub fn load(&self, editor: &mut Editor, path: &Path) -> Result<String, String> {
        let package = package::read_package(path, self.parse);
        let info = self.assess_and_load(editor, vec![package]).remove(0);
        if info.loadable() {
            Ok(info.name)
        } else {
            Err(failure(&info))
        }
    }

    /// Load every plugin in `dir` (package folders and `.wasm` files, by name). Returns what
    /// happened to each, including disabled and failed ones. A missing folder yields none.
    pub fn load_dir(&self, editor: &mut Editor, dir: &Path) -> Vec<PluginInfo> {
        let packages = package::discover(dir, self.parse);
        self.assess_and_load(editor, packages)
    }

    /// Health-check `packages` together, then load the ones that may load.
    fn assess_and_load(&self, editor: &mut Editor, packages: Vec<Package>) -> Vec<PluginInfo> {
        let names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
        let loaded: Vec<(String, Version)> = self
            .registry
            .borrow()
            .iter()
            .filter(|e| e.info.loadable() && !names.contains(&e.info.name))
            .map(|e| {
                (
                    e.info.name.clone(),
                    e.info.version().unwrap_or(Version {
                        major: 0,
                        minor: 0,
                        patch: 0,
                    }),
                )
            })
            .collect();
        let config: Vec<(String, Option<BTreeMap<String, Value>>)> = names
            .iter()
            .map(|n| (n.clone(), editor.plugin_config(n).cloned()))
            .collect();
        let lookup = |n: &str| {
            config
                .iter()
                .find(|(k, _)| k == n)
                .and_then(|(_, v)| v.as_ref())
        };
        let infos = package::assess(
            packages,
            &AssessEnv {
                vox: vox_version(),
                api: API_VERSION,
                loaded: &loaded,
                config: &lookup,
                program_exists: &health::program_on_path,
            },
        );
        infos
            .into_iter()
            .map(|info| self.load_info(editor, info))
            .collect()
    }

    /// Load one assessed plugin (if loadable), record it, and log the outcome.
    fn load_info(&self, editor: &mut Editor, mut info: PluginInfo) -> PluginInfo {
        let source = owner::plugin(&info.name);
        let mut instance = Weak::new();
        if info.loadable() {
            match self.instantiate(editor, &info) {
                Ok(inst) => {
                    instance = Rc::downgrade(&inst);
                    let custom = match call(&inst, editor, |b, s| b.call_health(s)) {
                        Ok(items) => items.into_iter().map(from_wit_health).collect(),
                        Err(e) => vec![HealthItem::problem(
                            Level::Error,
                            format!("health check: {e}"),
                        )],
                    };
                    info.add_items(custom);
                    if !info.loadable() {
                        // Its own health check failed: take back what it registered.
                        editor.remove_owned(&source);
                        instance = Weak::new();
                    }
                }
                Err(e) => info.add_items([HealthItem::problem(Level::Error, e)]),
            }
        }
        match info.status {
            HealthStatus::Disabled => editor.log(Level::Info, &source, "disabled in config"),
            HealthStatus::Failed => editor.log(
                Level::Error,
                &source,
                format!("not loaded: {}", failure(&info)),
            ),
            status => {
                let level = if status == HealthStatus::Warning {
                    Level::Warn
                } else {
                    Level::Info
                };
                let version = info.version().map_or(String::new(), |v| format!(" {v}"));
                editor.log(
                    level,
                    &source,
                    format!(
                        "loaded{version} from {} ({})",
                        info.path.display(),
                        status.name()
                    ),
                );
            }
        }
        let mut registry = self.registry.borrow_mut();
        registry.retain(|e| e.info.name != info.name);
        registry.push(Entry {
            info: info.clone(),
            instance,
        });
        info
    }

    /// Unload plugin `name` (remove everything it registered) and load it again from the
    /// same place, re-reading its manifest and settings.
    pub fn reload(&self, editor: &mut Editor, name: &str) -> Result<String, String> {
        let path = self
            .registry
            .borrow()
            .iter()
            .find(|e| e.info.name == name)
            .map(|e| e.info.path.clone())
            .ok_or_else(|| format!("no plugin named {name}"))?;
        editor.remove_owned(&owner::plugin(name));
        self.registry.borrow_mut().retain(|e| e.info.name != name);
        self.load(editor, &path)
    }

    /// Unload every plugin and load all plugins in `dir` again.
    pub fn reload_all(&self, editor: &mut Editor, dir: &Path) -> Vec<PluginInfo> {
        let names: Vec<String> = self
            .registry
            .borrow_mut()
            .drain(..)
            .map(|e| e.info.name)
            .collect();
        for n in names {
            editor.remove_owned(&owner::plugin(&n));
        }
        self.load_dir(editor, dir)
    }

    /// Names of all known plugins (loaded, failed, disabled), in load order.
    pub fn names(&self) -> Vec<String> {
        self.registry
            .borrow()
            .iter()
            .map(|e| e.info.name.clone())
            .collect()
    }

    /// What is known about each plugin: status, findings, settings.
    pub fn plugins(&self) -> Vec<PluginInfo> {
        self.registry
            .borrow()
            .iter()
            .map(|e| e.info.clone())
            .collect()
    }

    /// Run the custom `health()` check of every running plugin again and update its
    /// findings (the other checks ran at load time).
    fn recheck(&self, editor: &mut Editor) {
        let running: Vec<(String, Rc<RefCell<Instance>>)> = self
            .registry
            .borrow()
            .iter()
            .filter_map(|e| e.instance.upgrade().map(|i| (e.info.name.clone(), i)))
            .collect();
        for (name, inst) in running {
            let custom: Vec<HealthItem> = match call(&inst, editor, |b, s| b.call_health(s)) {
                Ok(items) => items.into_iter().map(from_wit_health).collect(),
                Err(e) => vec![HealthItem::problem(
                    Level::Error,
                    format!("health check: {e}"),
                )],
            };
            let mut registry = self.registry.borrow_mut();
            if let Some(entry) = registry.iter_mut().find(|e| e.info.name == name) {
                entry.info.items.retain(|i| !i.message.starts_with(CUSTOM));
                entry.info.add_items(custom);
            }
        }
    }

    /// Register `:reload plugins [NAME]` and `:checkhealth [NAME]` (owner `builtin`).
    pub fn register_commands(host: Rc<PluginHost>, editor: &mut Editor, dir: PathBuf) {
        let h = host.clone();
        editor.with_owner(owner::BUILTIN, |ed| {
            ed.commands_mut().register(
                "reload plugins",
                "Reload one plugin (`:reload plugins NAME`) or all plugins.",
                move |ed, args| {
                    let infos: Vec<Result<String, String>> = match args.arg {
                        Some(name) => vec![h.reload(ed, name)],
                        None => h
                            .reload_all(ed, &dir)
                            .into_iter()
                            .filter(|i| i.status != HealthStatus::Disabled)
                            .map(|i| {
                                if i.loadable() {
                                    Ok(i.name.clone())
                                } else {
                                    Err(failure(&i))
                                }
                            })
                            .collect(),
                    };
                    let ok = infos.iter().filter(|r| r.is_ok()).count();
                    let errors: Vec<String> = infos.into_iter().filter_map(Result::err).collect();
                    match errors.first() {
                        None => ed.notify(Level::Info, format!("reloaded {ok} plugin(s)")),
                        Some(e) => ed.notify(
                            Level::Error,
                            format!("reloaded {ok}, failed {}: {e}", errors.len()),
                        ),
                    }
                    Ok(())
                },
            );
            ed.commands_mut().register(
                "checkhealth",
                "Health report of all plugins, or one: `:checkhealth NAME`.",
                move |ed, args| {
                    host.recheck(ed);
                    let mut infos = host.plugins();
                    if let Some(name) = args.arg {
                        infos.retain(|i| i.name == name);
                        if infos.is_empty() {
                            return Err(format!("E900: no plugin named {name}"));
                        }
                    }
                    let lines = package::report(&infos)
                        .into_iter()
                        .map(|(key, text)| {
                            let style = match key {
                                "title" => Style::BOLD,
                                "ok" => Style::default().fg(Color::Green),
                                "warn" => Style::default().fg(Color::Yellow),
                                "error" => Style::default().fg(Color::Red),
                                _ => Style::default(),
                            };
                            (style, text)
                        })
                        .collect();
                    ed.open_modal(Box::new(TextView::new("Plugin health", lines)));
                    Ok(())
                },
            );
        });
    }

    /// Compile and instantiate a plugin, give it its settings, and call `init`.
    fn instantiate(
        &self,
        editor: &mut Editor,
        info: &PluginInfo,
    ) -> Result<Rc<RefCell<Instance>>, String> {
        let name = info.name.clone();
        let fail = |e: wasmtime::Error| format!("plugin {name}: {e}");
        let component = Component::from_file(&self.engine, &info.wasm).map_err(fail)?;
        let mut store = Store::new(
            &self.engine,
            HostState {
                name: name.clone(),
                editor: EditorPtr(std::ptr::null_mut()),
                requests: Vec::new(),
                settings: info.settings.clone(),
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
        Ok(plugin)
    }
}

/// Prefix of findings from a plugin's own `health()`.
const CUSTOM: &str = "plugin check: ";

/// WIT health item → core finding.
fn from_wit_health(item: wit::HealthItem) -> HealthItem {
    let level = to_level(item.level);
    let message = format!("{CUSTOM}{}", item.message);
    match level {
        Level::Debug | Level::Info => HealthItem::ok(message),
        _ => HealthItem::problem(level, message),
    }
}

/// The reason a plugin is not loaded: its first error finding.
fn failure(info: &PluginInfo) -> String {
    let reason = info
        .items
        .iter()
        .find(|i| !i.ok && i.level == Level::Error)
        .map_or("unknown reason".to_string(), |i| i.message.clone());
    format!("plugin {}: {reason}", info.name)
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
    /// Plugin name, used as the log source.
    name: String,
    editor: EditorPtr,
    requests: Vec<Request>,
    /// Effective settings from the manifest and `[plugins.NAME]`.
    settings: Vec<(String, vox_editor::plugin::SettingValue)>,
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
    let owner = match plugin.try_borrow() {
        Ok(inst) => owner::plugin(&inst.name),
        Err(_) => return Err("plugin is already running (it called itself)".to_string()),
    };
    editor.with_owner(&owner, |editor| call_owned(plugin, editor, f))
}

/// [`call`] with the plugin already set as owner of anything registered.
fn call_owned<R>(
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
    result.map_err(|e| {
        let msg = describe(&name, &e);
        editor.log(Level::Error, &owner::plugin(&name), &msg);
        msg
    })
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

/// WIT level → core level.
fn to_level(l: wit::Level) -> Level {
    match l {
        wit::Level::Debug => Level::Debug,
        wit::Level::Info => Level::Info,
        wit::Level::Warn => Level::Warn,
        wit::Level::Error => Level::Error,
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

    /// WIT `notify`: show a notification.
    fn notify(&mut self, level: wit::Level, text: String) {
        self.editor().notify(to_level(level), text);
    }

    /// WIT `log`: add a log line with this plugin as the source.
    fn log(&mut self, level: wit::Level, text: String) {
        let source = owner::plugin(&self.name);
        self.editor().log(to_level(level), &source, text);
    }

    /// WIT `setting`: the value of one of this plugin's settings.
    fn setting(&mut self, key: String) -> Option<wit::SettingValue> {
        use vox_editor::plugin::SettingValue as V;
        self.settings
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| match v {
                V::String(s) => wit::SettingValue::Text(s.clone()),
                V::Integer(n) => wit::SettingValue::Integer(*n),
                V::Bool(b) => wit::SettingValue::Boolean(*b),
            })
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
