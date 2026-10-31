//! Shared foundations for Sulfur: where things live on disk and the persisted
//! user settings. Kept free of networking and UI concerns so every other crate
//! (and any future CLI) can depend on it.

mod accounts;
pub mod import;
mod paths;
mod settings;
pub mod wallpaper;

pub use accounts::{Accounts, MicrosoftAccount, OfflineAccount, check_offline_name};
pub use paths::AppDirs;
pub use settings::{
    AccountSettings, AppearanceSettings, ColorSource, DownloadSettings, DownloadSource, GameSettings, GarbageCollector, Isolation,
    LaunchColumn, LaunchPageSettings, LaunchSettings, LaunchWidget, ModSettings, NavigationPosition, Settings, SettingsError,
    ThemeMode, VersionSettings, WidgetKind,
};
