//! Write vox plugins in Rust.
//!
//! A plugin is a type implementing [`Plugin`], exported with [`export_plugin!`], built for the
//! `wasm32-wasip2` target as a `cdylib`, and shipped with a `plugin.toml` manifest. The
//! contract is `wit/vox.wit`. Start a new plugin with `:plugin new NAME DIR` in vox.
//!
//! ```ignore
//! use vox_plugin_sdk::{CommandInfo, Plugin, editor, export_plugin};
//!
//! struct Hello;
//!
//! impl Plugin for Hello {
//!     fn init() -> Result<(), String> {
//!         editor::register_command("hello", "Say hello");
//!         // Optional: help for `:help hello` and completion.
//!         editor::describe_command(
//!             "hello",
//!             &CommandInfo::new().example("hello", "show the greeting"),
//!         );
//!         Ok(())
//!     }
//!     fn run_command(name: String, _arg: Option<String>, _force: bool) -> Result<(), String> {
//!         let text = editor::setting("greeting").and_then(|v| v.as_text());
//!         editor::message(&text.unwrap_or_else(|| "hello".into()));
//!         Ok(())
//!     }
//! }
//!
//! export_plugin!(Hello);
//! ```
//!
//! Uses the `wit-bindgen` library to generate the WebAssembly glue code.

/// Code generated from `wit/vox.wit`. Use the re-exports at the crate root instead.
#[doc(hidden)]
#[allow(missing_docs, clippy::all)]
pub mod bindings {
    wit_bindgen::generate!({
        world: "plugin",
        path: "../../wit",
        pub_export_macro: true,
        export_macro_name: "__export_plugin",
        default_bindings_module: "vox_plugin_sdk::bindings",
    });
}

/// Functions the plugin can call: register commands, map keys, read and edit the text,
/// read settings, log.
pub use bindings::vox::plugin::editor;
pub use bindings::vox::plugin::types::{
    CommandInfo, Completion, Event, EventKind, Example, HealthItem, Level, MapMode, Mode,
    ParamInfo, Position, SettingValue,
};

/// What a plugin implements. `on_event` and `health` are optional.
pub trait Plugin {
    /// Called once at load: register commands, mappings, and event subscriptions.
    fn init() -> Result<(), String>;
    /// Run one of this plugin's commands. An `Err` is shown to the user.
    fn run_command(name: String, arg: Option<String>, force: bool) -> Result<(), String>;
    /// Handle an event this plugin subscribed to.
    fn on_event(_event: Event) {}
    /// Custom health checks (after `init`, and in `:checkhealth`). An `Error` item unloads
    /// the plugin.
    fn health() -> Vec<HealthItem> {
        Vec::new()
    }
}

impl SettingValue {
    /// The text, if this is a text (or choice) setting.
    pub fn as_text(&self) -> Option<String> {
        match self {
            SettingValue::Text(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// The number, if this is an integer setting.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            SettingValue::Integer(n) => Some(*n),
            _ => None,
        }
    }

    /// The flag, if this is a bool setting.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            SettingValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

impl CommandInfo {
    /// Help with nothing set: category `Plugin: NAME`, no parameters.
    pub fn new() -> Self {
        Self {
            category: None,
            params: Vec::new(),
            returns: None,
            force: None,
            examples: Vec::new(),
        }
    }

    /// Set the category the command is listed under.
    pub fn category(mut self, category: &str) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Add a parameter.
    pub fn param(mut self, param: ParamInfo) -> Self {
        self.params.push(param);
        self
    }

    /// Describe what the command replies with.
    pub fn returns(mut self, returns: &str) -> Self {
        self.returns = Some(returns.into());
        self
    }

    /// Describe what `:name!` does.
    pub fn force(mut self, force: &str) -> Self {
        self.force = Some(force.into());
        self
    }

    /// Add an example command line (without `:`) and what it does.
    pub fn example(mut self, line: &str, note: &str) -> Self {
        self.examples.push(Example {
            line: line.into(),
            note: note.into(),
        });
        self
    }
}

impl Default for CommandInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ParamInfo {
    /// A parameter that must be given.
    pub fn required(name: &str, description: &str) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            required: true,
            repeated: false,
            complete: Completion::None,
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
    pub fn complete(mut self, complete: Completion) -> Self {
        self.complete = complete;
        self
    }

    /// Complete from a fixed set of words.
    pub fn values(self, values: &[&str]) -> Self {
        self.complete(Completion::Values(
            values.iter().map(|v| v.to_string()).collect(),
        ))
    }
}

/// Export `$t` (the name of a type implementing [`Plugin`]) as this plugin.
#[macro_export]
macro_rules! export_plugin {
    ($t:ident) => {
        #[doc(hidden)]
        struct __VoxPluginExport;

        impl $crate::bindings::Guest for __VoxPluginExport {
            /// Forwards to [`Plugin::init`].
            fn init() -> Result<(), String> {
                <$t as $crate::Plugin>::init()
            }
            /// Forwards to [`Plugin::run_command`].
            fn run_command(name: String, arg: Option<String>, force: bool) -> Result<(), String> {
                <$t as $crate::Plugin>::run_command(name, arg, force)
            }
            /// Forwards to [`Plugin::on_event`].
            fn on_event(event: $crate::Event) {
                <$t as $crate::Plugin>::on_event(event)
            }
            /// Forwards to [`Plugin::health`].
            fn health() -> Vec<$crate::HealthItem> {
                <$t as $crate::Plugin>::health()
            }
        }

        $crate::bindings::__export_plugin!(__VoxPluginExport);
    };
}
