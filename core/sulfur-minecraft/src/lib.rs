//! Minecraft: Java Edition support for Sulfur.
//!
//! * [`manifest`], [`version`], [`assets`]: Mojang's metadata formats.
//! * [`library`], [`rules`], [`platform`]: deciding which files apply to this machine.
//! * [`install::Installer`]: turns a version into one parallel download batch.
//! * [`launch`], [`process`], [`java`], [`account`]: building and running the game command.
//! * [`loader`]: Fabric, Legacy Fabric, Ornithe, Quilt, Forge, NeoForge, Cleanroom, Rift and OptiFine.
//! * [`runtime`]: Java runtimes downloaded on demand.
//! * [`groups`], [`banners`]: the version list as major updates with artwork.
//! * [`news`]: Java Edition news from the official launcher's feed.
//! * [`omniarchive`]: lost versions preserved by Omniarchive.
//! * [`region`]: where offline accounts are allowed.
//! * [`msa`]: Microsoft account sign-in.
//! * [`alkaline`]: the Alkaline client, found or built and started.

pub mod account;
pub mod alkaline;
pub mod assets;
pub mod banners;
pub mod groups;
pub mod install;
pub mod java;
pub mod launch;
pub mod layout;
pub mod library;
pub mod loader;
pub mod manifest;
pub mod msa;
pub mod news;
pub mod omniarchive;
pub mod platform;
pub mod process;
pub mod region;
pub mod rules;
pub mod runtime;
pub mod sources;
pub mod version;

mod error;

pub use error::McError;

pub type Result<T, E = McError> = std::result::Result<T, E>;
