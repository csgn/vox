//! Owners: who added a command, key mapping, or event handler. Used to remove exactly
//! those things again, e.g. when reloading the config or a plugin.

/// Built into vox.
pub const BUILTIN: &str = "builtin";
/// From the user's config file.
pub const CONFIG: &str = "config";
/// vox's default key mappings (e.g. `<leader>p`), re-made when the leader changes.
pub const DEFAULTS: &str = "defaults";
/// Developer mode's default mappings.
pub const DEVMODE: &str = "devmode";
/// Agent mode's commands and mappings, removed in text mode.
pub const AGENT: &str = "agent";
/// Added through the API without a more specific owner (the default).
pub const USER: &str = "user";

/// Owner name of plugin `name`: `plugin:NAME`.
pub fn plugin(name: &str) -> String {
    format!("plugin:{name}")
}
