//! Microsoft account sign-in for Minecraft: Java Edition.
//!
//! The chain every launcher follows, with the OAuth device code flow so no
//! browser redirect or local web server is needed:
//!
//! 1. Microsoft identity platform: device code, then a Microsoft access and
//!    refresh token (`XboxLive.signin offline_access`).
//! 2. Xbox Live user token (`user.auth.xboxlive.com`), with the user hash.
//! 3. XSTS token for `rp://api.minecraftservices.com/`.
//! 4. Minecraft access token (`login_with_xbox`), valid for about a day.
//! 5. The Minecraft profile (name, UUID, skin), which only exists for players
//!    who own the game (bought or through Game Pass).
//!
//! The application (client) ID must be registered in Microsoft Entra and
//! approved by Mojang for the Minecraft API; without approval step 4 answers
//! 403.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{Value, json};
use sulfur_download::{CancellationToken, encode_component};

const SCOPE: &str = "XboxLive.signin offline_access";

/// Where each step of the sign-in goes; overridable for tests.
#[derive(Clone, Debug)]
pub struct Endpoints {
    pub device_code: String,
    pub token: String,
    pub xbox_user: String,
    pub xsts: String,
    pub minecraft_login: String,
    pub profile: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            device_code: "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode".into(),
            token: "https://login.microsoftonline.com/consumers/oauth2/v2.0/token".into(),
            xbox_user: "https://user.auth.xboxlive.com/user/authenticate".into(),
            xsts: "https://xsts.auth.xboxlive.com/xsts/authorize".into(),
            minecraft_login: "https://api.minecraftservices.com/authentication/login_with_xbox".into(),
            profile: "https://api.minecraftservices.com/minecraft/profile".into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Microsoft sign-in is not configured in this build of Sulfur")]
    NotConfigured,
    #[error("network error during sign-in: {0}")]
    Network(#[from] reqwest::Error),
    #[error("the sign-in code expired; start again")]
    Expired,
    #[error("sign-in was declined")]
    Declined,
    #[error("sign-in cancelled")]
    Cancelled,
    #[error("this Microsoft account has no Xbox profile yet; sign in once at xbox.com to create one")]
    NoXboxAccount,
    #[error("Xbox Live is not available in this account's country or region")]
    XboxUnavailable,
    #[error("this account needs adult verification on xbox.com before it can play")]
    AdultVerification,
    #[error("this is a child account; an adult must add it to a Microsoft family first")]
    ChildAccount,
    #[error("this account does not own Minecraft: Java Edition")]
    NotOwned,
    #[error("Mojang has not approved this build's application ID for Minecraft sign-in")]
    AppNotApproved,
    #[error("the saved sign-in is no longer valid; sign in again")]
    Revoked,
    #[error("{step} failed: HTTP {status}: {body}")]
    Http { step: &'static str, status: u16, body: String },
    #[error("unexpected answer from {0}")]
    Malformed(&'static str),
}

/// What the player has to do to approve the sign-in.
#[derive(Clone, Debug, Deserialize)]
pub struct DeviceCode {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    /// Seconds until the code expires.
    pub expires_in: u64,
    /// Seconds to wait between polls.
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

/// Microsoft OAuth tokens.
#[derive(Clone, Debug)]
pub struct MsaTokens {
    pub access_token: String,
    pub refresh_token: String,
}

/// A signed-in Minecraft player.
#[derive(Clone, Debug)]
pub struct MinecraftSession {
    pub access_token: String,
    /// Unix seconds.
    pub expires_at: u64,
    /// UUID without dashes.
    pub uuid: String,
    pub name: String,
    pub xuid: String,
    pub skin_url: Option<String>,
}

/// Steps reported while turning a Microsoft token into a Minecraft session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Xbox,
    Minecraft,
    Profile,
}

pub struct MsaClient {
    http: reqwest::Client,
    client_id: String,
    endpoints: Endpoints,
}

impl MsaClient {
    pub fn new(http: reqwest::Client, client_id: impl Into<String>) -> Self {
        Self::with_endpoints(http, client_id, Endpoints::default())
    }

    pub fn with_endpoints(http: reqwest::Client, client_id: impl Into<String>, endpoints: Endpoints) -> Self {
        Self {
            http,
            client_id: client_id.into(),
            endpoints,
        }
    }

    /// Starts a sign-in: the player opens `verification_uri` and enters `user_code`.
    pub async fn request_device_code(&self) -> Result<DeviceCode, AuthError> {
        let body = form(&[("client_id", &self.client_id), ("scope", SCOPE)]);
        let (status, value) = self.post_form(&self.endpoints.device_code, body).await?;
        if status != 200 {
            return Err(http_error("device code request", status, &value));
        }
        serde_json::from_value(value).map_err(|_| AuthError::Malformed("device code request"))
    }

    /// Waits until the player approves (or declines) the sign-in.
    pub async fn poll_device_code(&self, code: &DeviceCode, cancel: &CancellationToken) -> Result<MsaTokens, AuthError> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(code.expires_in);
        let mut interval = Duration::from_secs(code.interval.max(1));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return Err(AuthError::Cancelled),
                _ = tokio::time::sleep(interval) => {}
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(AuthError::Expired);
            }
            let body = form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", &self.client_id),
                ("device_code", &code.device_code),
            ]);
            let (status, value) = self.post_form(&self.endpoints.token, body).await?;
            if status == 200 {
                return tokens(&value);
            }
            match value["error"].as_str() {
                Some("authorization_pending") => {}
                Some("slow_down") => interval += Duration::from_secs(5),
                Some("expired_token") => return Err(AuthError::Expired),
                Some("authorization_declined" | "access_denied") => return Err(AuthError::Declined),
                _ => return Err(http_error("token request", status, &value)),
            }
        }
    }

    /// New Microsoft tokens from a saved refresh token.
    pub async fn refresh(&self, refresh_token: &str) -> Result<MsaTokens, AuthError> {
        let body = form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &self.client_id),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ]);
        let (status, value) = self.post_form(&self.endpoints.token, body).await?;
        match status {
            200 => tokens(&value),
            400 if value["error"] == "invalid_grant" => Err(AuthError::Revoked),
            _ => Err(http_error("token refresh", status, &value)),
        }
    }

    /// Xbox Live, XSTS, Minecraft token and profile from a Microsoft access token.
    pub async fn minecraft_session(
        &self,
        msa_access_token: &str,
        mut on_step: impl FnMut(Step),
    ) -> Result<MinecraftSession, AuthError> {
        on_step(Step::Xbox);
        let (status, user) = self
            .post_json(
                &self.endpoints.xbox_user,
                json!({
                    "Properties": {
                        "AuthMethod": "RPS",
                        "SiteName": "user.auth.xboxlive.com",
                        "RpsTicket": format!("d={msa_access_token}"),
                    },
                    "RelyingParty": "http://auth.xboxlive.com",
                    "TokenType": "JWT",
                }),
            )
            .await?;
        if status != 200 {
            return Err(xbox_error("Xbox Live sign-in", status, &user));
        }
        let user_token = user["Token"].as_str().ok_or(AuthError::Malformed("Xbox Live"))?;

        let (status, xsts) = self
            .post_json(
                &self.endpoints.xsts,
                json!({
                    "Properties": { "SandboxId": "RETAIL", "UserTokens": [user_token] },
                    "RelyingParty": "rp://api.minecraftservices.com/",
                    "TokenType": "JWT",
                }),
            )
            .await?;
        if status != 200 {
            return Err(xbox_error("XSTS authorization", status, &xsts));
        }
        let xsts_token = xsts["Token"].as_str().ok_or(AuthError::Malformed("XSTS"))?;
        let claims = &xsts["DisplayClaims"]["xui"][0];
        let user_hash = claims["uhs"].as_str().ok_or(AuthError::Malformed("XSTS"))?;
        let xuid = claims["xid"].as_str().unwrap_or("0").to_owned();

        on_step(Step::Minecraft);
        let (status, login) = self
            .post_json(
                &self.endpoints.minecraft_login,
                json!({ "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}") }),
            )
            .await?;
        match status {
            200 => {}
            // Mojang answers 403 "Invalid app registration" for client IDs it has not approved.
            403 => return Err(AuthError::AppNotApproved),
            _ => return Err(http_error("Minecraft sign-in", status, &login)),
        }
        let access_token = login["access_token"].as_str().ok_or(AuthError::Malformed("Minecraft sign-in"))?.to_owned();
        let expires_in = login["expires_in"].as_u64().unwrap_or(86_400);

        on_step(Step::Profile);
        let response = self.http.get(&self.endpoints.profile).bearer_auth(&access_token).send().await?;
        let status = response.status().as_u16();
        let profile = parse(&response.bytes().await?);
        match status {
            200 => {}
            // No profile: the account does not own Java Edition (or never set a name).
            404 => return Err(AuthError::NotOwned),
            _ => return Err(http_error("Minecraft profile", status, &profile)),
        }
        let uuid = profile["id"].as_str().ok_or(AuthError::Malformed("Minecraft profile"))?.replace('-', "");
        let name = profile["name"].as_str().ok_or(AuthError::Malformed("Minecraft profile"))?.to_owned();
        let skin_url = profile["skins"]
            .as_array()
            .and_then(|skins| skins.iter().find(|s| s["state"] == "ACTIVE").or(skins.first()))
            .and_then(|s| s["url"].as_str())
            .map(|url| url.replacen("http://", "https://", 1));
        Ok(MinecraftSession {
            access_token,
            expires_at: now() + expires_in,
            uuid,
            name,
            xuid,
            skin_url,
        })
    }

    async fn post_form(&self, url: &str, body: String) -> Result<(u16, Value), AuthError> {
        let response = self
            .http
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await?;
        let status = response.status().as_u16();
        Ok((status, parse(&response.bytes().await?)))
    }

    async fn post_json(&self, url: &str, body: Value) -> Result<(u16, Value), AuthError> {
        let response = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body.to_string())
            .send()
            .await?;
        let status = response.status().as_u16();
        Ok((status, parse(&response.bytes().await?)))
    }
}

/// Seconds since the Unix epoch.
pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", encode_component(k), encode_component(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn parse(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap_or(Value::Null)
}

fn tokens(value: &Value) -> Result<MsaTokens, AuthError> {
    Ok(MsaTokens {
        access_token: value["access_token"].as_str().ok_or(AuthError::Malformed("token request"))?.to_owned(),
        refresh_token: value["refresh_token"].as_str().ok_or(AuthError::Malformed("token request"))?.to_owned(),
    })
}

fn http_error(step: &'static str, status: u16, body: &Value) -> AuthError {
    let detail = body["error_description"]
        .as_str()
        .or(body["errorMessage"].as_str())
        .or(body["error"].as_str())
        .unwrap_or_default();
    AuthError::Http {
        step,
        status,
        body: detail.chars().take(300).collect(),
    }
}

/// Xbox Live explains refusals with an `XErr` code.
fn xbox_error(step: &'static str, status: u16, body: &Value) -> AuthError {
    match body["XErr"].as_u64() {
        Some(2148916233) => AuthError::NoXboxAccount,
        Some(2148916235) => AuthError::XboxUnavailable,
        Some(2148916236 | 2148916237) => AuthError::AdultVerification,
        Some(2148916238) => AuthError::ChildAccount,
        _ => http_error(step, status, body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_xbox_refusals() {
        assert!(matches!(xbox_error("x", 401, &json!({"XErr": 2148916233u64})), AuthError::NoXboxAccount));
        assert!(matches!(xbox_error("x", 401, &json!({"XErr": 2148916238u64})), AuthError::ChildAccount));
        assert!(matches!(xbox_error("x", 500, &json!({})), AuthError::Http { status: 500, .. }));
    }

    #[test]
    fn encodes_forms() {
        assert_eq!(form(&[("scope", SCOPE), ("a", "b&c")]), "scope=XboxLive.signin%20offline_access&a=b%26c");
    }
}
