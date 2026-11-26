//! Plugin packages: manifests (`plugin.toml`), versions, settings, health checks, and the
//! project template. Running plugins is done by the `vox-plugin-host` adapter.

pub mod health;
pub mod install;
pub mod lock;
pub mod manage;
pub mod manifest;
pub mod package;
pub mod sha256;
pub mod template;
pub mod version;

pub use health::{HealthEnv, HealthItem, HealthStatus};
pub use manifest::{Need, PluginManifest, Requirement, SettingKind, SettingSpec, SettingValue};
pub use package::{AssessEnv, Package, PluginInfo};
pub use version::{Version, VersionReq};

/// The plugin API version vox provides (the package version in `wit/vox.wit`).
pub const API_VERSION: Version = Version {
    major: 0,
    minor: 2,
    patch: 0,
};
