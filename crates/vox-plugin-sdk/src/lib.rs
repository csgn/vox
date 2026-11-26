//! Write vox plugins in Rust.
//!
//! A plugin is a type implementing [`Plugin`], exported with [`export_plugin!`], and built
//! for the `wasm32-wasip2` target as a `cdylib`. The contract is `wit/vox.wit`.
//!
//! ```ignore
//! use vox_plugin_sdk::{Event, Plugin, editor, export_plugin};
//!
//! struct Hello;
//!
//! impl Plugin for Hello {
//!     fn init() -> Result<(), String> {
//!         editor::register_command("hello", "Say hello");
//!         Ok(())
//!     }
//!     fn run_command(name: String, _arg: Option<String>, _force: bool) -> Result<(), String> {
//!         match name.as_str() {
//!             "hello" => editor::message("hello from a plugin"),
//!             _ => {}
//!         }
//!         Ok(())
//!     }
//!     fn on_event(_event: Event) {}
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

/// What a plugin implements: `init`, `run_command`, `on_event`.
pub use bindings::Guest as Plugin;
/// Functions the plugin can call: register commands, map keys, read and edit the text.
pub use bindings::vox::plugin::editor;
pub use bindings::vox::plugin::types::{Event, EventKind, MapMode, Mode, Position};

/// Export `$t` (the name of a type implementing [`Plugin`]) as this plugin.
#[macro_export]
macro_rules! export_plugin {
    ($t:ident) => {
        $crate::bindings::__export_plugin!($t);
    };
}
