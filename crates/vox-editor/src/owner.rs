//! Owners: who added a command, key mapping, or event handler. Used to remove exactly
//! those things again, e.g. when reloading the config or a plugin.

/// Built into vox.
pub const BUILTIN: &str = "builtin";
/// From the user's config file.
pub const CONFIG: &str = "config";
/// Developer mode's default mappings.
pub const DEVMODE: &str = "devmode";
/// Added through the API without a more specific owner (the default).
pub const USER: &str = "user";

/// Owner name of plugin `name`: `plugin:NAME`.
pub fn plugin(name: &str) -> String {
    format!("plugin:{name}")
}
