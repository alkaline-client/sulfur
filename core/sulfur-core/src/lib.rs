//! Shared foundations for Sulfur: where things live on disk and the persisted
//! user settings. Kept free of networking and UI concerns so every other crate
//! (and any future CLI) can depend on it.

mod accounts;
pub mod import;
mod paths;
mod settings;

pub use accounts::{Accounts, MicrosoftAccount};
pub use paths::AppDirs;
pub use settings::{
    AccountSettings, AppearanceSettings, DownloadSettings, DownloadSource, GameSettings, Isolation, ModSettings, NavigationPosition,
    Settings, SettingsError, ThemeMode,
};
