//! Accounts, kept apart from the settings because Microsoft accounts hold
//! tokens: `accounts.json` in the config folder, readable only by the user
//! on Unix. Offline accounts are just player names.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A Microsoft account that owns Minecraft: Java Edition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicrosoftAccount {
    /// Minecraft UUID without dashes; identifies the account.
    pub uuid: String,
    pub name: String,
    pub xuid: String,
    pub skin_url: Option<String>,
    /// Microsoft OAuth refresh token, used to sign in again without the player.
    pub refresh_token: String,
    /// Minecraft access token and when it expires (Unix seconds).
    pub access_token: String,
    pub expires_at: u64,
}

/// A player name for offline play.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineAccount {
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Accounts {
    pub microsoft: Vec<MicrosoftAccount>,
    pub offline: Vec<OfflineAccount>,
}

impl Accounts {
    pub fn file(config_dir: &Path) -> PathBuf {
        config_dir.join("accounts.json")
    }

    /// Loads the accounts; a missing or unreadable file means none.
    pub fn load(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "ignoring invalid accounts file");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Writes the accounts atomically, with owner-only permissions on Unix.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        std::io::Write::write_all(&mut options.open(&tmp)?, &text)?;
        std::fs::rename(&tmp, path)
    }

    pub fn find(&self, uuid: &str) -> Option<&MicrosoftAccount> {
        self.microsoft.iter().find(|a| a.uuid == uuid)
    }

    /// Adds an account or replaces the one with the same UUID.
    pub fn upsert(&mut self, account: MicrosoftAccount) {
        match self.microsoft.iter_mut().find(|a| a.uuid == account.uuid) {
            Some(existing) => *existing = account,
            None => self.microsoft.push(account),
        }
    }

    pub fn remove(&mut self, uuid: &str) {
        self.microsoft.retain(|a| a.uuid != uuid);
    }

    /// The offline account with this name, ignoring case as servers do.
    pub fn find_offline(&self, name: &str) -> Option<&OfflineAccount> {
        self.offline.iter().find(|a| a.name.eq_ignore_ascii_case(name))
    }
}

/// Why a name cannot be an offline player name: empty, longer than 16
/// characters (the game's limit) or containing spaces or control characters.
/// Other characters work in single player and on most offline servers.
pub fn check_offline_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        Err("enter a name")
    } else if name.chars().count() > 16 {
        Err("names have at most 16 characters")
    } else if name.chars().any(|c| c.is_whitespace() || c.is_control()) {
        Err("names cannot contain spaces")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_replaces_accounts() {
        let dir = std::env::temp_dir().join(format!("sulfur-accounts-{}", std::process::id()));
        let path = Accounts::file(&dir);
        let account = MicrosoftAccount {
            uuid: "abc".into(),
            name: "Steve".into(),
            xuid: "1".into(),
            skin_url: None,
            refresh_token: "r".into(),
            access_token: "a".into(),
            expires_at: 10,
        };
        let mut accounts = Accounts::default();
        accounts.upsert(account.clone());
        accounts.upsert(MicrosoftAccount { name: "Alex".into(), ..account });
        assert_eq!(accounts.microsoft.len(), 1);
        accounts.offline.push(OfflineAccount { name: "Steve".into() });
        assert!(accounts.find_offline("steve").is_some());
        accounts.save(&path).unwrap();
        assert_eq!(Accounts::load(&path), accounts);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn older_files_have_no_offline_accounts() {
        let accounts: Accounts = serde_json::from_str(r#"{"microsoft":[]}"#).unwrap();
        assert!(accounts.offline.is_empty());
    }

    #[test]
    fn checks_offline_names() {
        assert!(check_offline_name("Steve_01").is_ok());
        assert!(check_offline_name("史蒂夫").is_ok());
        assert!(check_offline_name("").is_err());
        assert!(check_offline_name("a b").is_err());
        assert!(check_offline_name("abcdefghijklmnopq").is_err());
    }
}
