//! Player identities passed to the game.

use md5::{Digest, Md5};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountKind {
    Offline,
    Microsoft,
}

#[derive(Clone, Debug)]
pub struct Account {
    pub kind: AccountKind,
    pub name: String,
    /// UUID without dashes.
    pub uuid: String,
    pub access_token: String,
    pub xuid: String,
}

impl Account {
    /// An offline account with the same UUID the vanilla server assigns.
    pub fn offline(name: &str) -> Self {
        Self {
            kind: AccountKind::Offline,
            name: name.to_owned(),
            uuid: offline_uuid(name),
            access_token: "0".into(),
            xuid: "0".into(),
        }
    }

    /// A Microsoft account signed in to Minecraft.
    pub fn microsoft(name: &str, uuid: &str, access_token: &str, xuid: &str) -> Self {
        Self {
            kind: AccountKind::Microsoft,
            name: name.to_owned(),
            uuid: uuid.to_owned(),
            access_token: access_token.to_owned(),
            xuid: xuid.to_owned(),
        }
    }

    pub fn user_type(&self) -> &'static str {
        match self.kind {
            AccountKind::Offline => "legacy",
            AccountKind::Microsoft => "msa",
        }
    }
}

/// Whether offline accounts may be used: in mainland China (where Java
/// Edition is not sold) always, elsewhere only next to a Microsoft account
/// that owns the game. There is deliberately no override.
pub fn offline_allowed(region: crate::region::Region, owns_game: bool) -> bool {
    region == crate::region::Region::MainlandChina || owns_game
}

/// Java's `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)`, without dashes.
pub fn offline_uuid(name: &str) -> String {
    let mut bytes: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}")).into();
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_vanilla_offline_uuid() {
        // Known value for "Notch" from vanilla offline-mode servers.
        assert_eq!(super::offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
    }
}
