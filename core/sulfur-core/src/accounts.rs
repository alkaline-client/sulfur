//! Signed-in Microsoft accounts, kept apart from the settings because they
//! hold tokens: `accounts.json` in the config folder, readable only by the
//! user on Unix.

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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Accounts {
    pub microsoft: Vec<MicrosoftAccount>,
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
        accounts.save(&path).unwrap();
        assert_eq!(Accounts::load(&path), accounts);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
