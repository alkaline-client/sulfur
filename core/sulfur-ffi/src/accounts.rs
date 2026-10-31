//! Accounts: the offline player and signed-in Microsoft accounts.

use std::sync::{Arc, Mutex};

use sulfur_core::{Accounts, MicrosoftAccount};
use sulfur_download::CancellationToken;
use sulfur_minecraft::{
    account::Account,
    msa::{self, AuthError, MsaClient, Step},
};

use crate::{AccountInfo, AccountKind, CoreError, Result, SignInSnapshot, SignInStage, State, SulfurCore};

/// Application (client) ID registered for Sulfur in Microsoft Entra. Set at
/// build time (`SULFUR_MSA_CLIENT_ID`, or `sulfur.msaClientId` as a Gradle
/// property or in `local.properties`); the same variable at run time
/// overrides it for development. An empty value counts as unset.
fn client_id() -> Option<String> {
    let set = |id: &str| Some(id.trim().to_owned()).filter(|id| !id.is_empty());
    std::env::var("SULFUR_MSA_CLIENT_ID")
        .ok()
        .and_then(|id| set(&id))
        .or_else(|| option_env!("SULFUR_MSA_CLIENT_ID").and_then(set))
}

pub(crate) fn microsoft_available() -> bool {
    client_id().is_some()
}

/// A Minecraft token this close to expiring is renewed before launching.
const RENEW_MARGIN_SECS: u64 = 10 * 60;

impl From<&MicrosoftAccount> for AccountInfo {
    fn from(a: &MicrosoftAccount) -> Self {
        Self {
            id: a.uuid.clone(),
            kind: AccountKind::Microsoft,
            name: a.name.clone(),
            skin_url: a.skin_url.clone(),
        }
    }
}

impl State {
    fn msa(&self) -> Result<MsaClient> {
        let id = client_id().ok_or_else(|| CoreError::Account(AuthError::NotConfigured.to_string()))?;
        Ok(MsaClient::new(self.downloader.read().unwrap().http_client(), id))
    }

    fn save_accounts(&self, accounts: &Accounts) -> Result<()> {
        accounts
            .save(&Accounts::file(&self.dirs.config))
            .map_err(|e| CoreError::Io(format!("cannot save accounts: {e}")))
    }

    pub(crate) fn owns_game(&self) -> bool {
        !self.accounts.lock().unwrap().microsoft.is_empty()
    }

    pub(crate) fn offline_allowed(&self) -> bool {
        sulfur_minecraft::account::offline_allowed(self.region, self.owns_game())
    }

    /// The account to play with: the selected one if it still exists, else
    /// offline where allowed, else the first Microsoft account.
    pub(crate) fn current_account(&self) -> Option<AccountInfo> {
        let settings = self.settings.read().unwrap();
        let accounts = self.accounts.lock().unwrap();
        let offline = || AccountInfo {
            id: String::new(),
            kind: AccountKind::Offline,
            name: settings.account.offline_name.clone(),
            skin_url: None,
        };
        let offline_allowed = sulfur_minecraft::account::offline_allowed(self.region, !accounts.microsoft.is_empty());
        let selected = &settings.account.selected;
        if let Some(account) = accounts.find(selected).filter(|_| !selected.is_empty()) {
            return Some(account.into());
        }
        if offline_allowed {
            return Some(offline());
        }
        accounts.microsoft.first().map(Into::into)
    }

    /// The account to launch with, renewing a Microsoft account's token when needed.
    pub(crate) async fn launch_account(&self) -> Result<Account> {
        let Some(info) = self.current_account() else {
            return Err(CoreError::Account(if self.offline_allowed() {
                "no account selected".into()
            } else {
                "sign in with a Microsoft account that owns Minecraft to play".into()
            }));
        };
        if info.kind == AccountKind::Offline {
            return Ok(Account::offline(&info.name));
        }
        let stored = self.accounts.lock().unwrap().find(&info.id).cloned().ok_or_else(|| CoreError::Account("account not found".into()))?;
        let account = if stored.expires_at > msa::now() + RENEW_MARGIN_SECS {
            stored
        } else {
            self.renew(stored).await?
        };
        Ok(Account::microsoft(&account.name, &account.uuid, &account.access_token, &account.xuid))
    }

    /// Signs in again with the saved refresh token.
    async fn renew(&self, account: MicrosoftAccount) -> Result<MicrosoftAccount> {
        let client = self.msa()?;
        let tokens = client.refresh(&account.refresh_token).await.map_err(auth_error)?;
        let session = client.minecraft_session(&tokens.access_token, |_| {}).await.map_err(auth_error)?;
        let renewed = stored_account(session, tokens.refresh_token);
        let mut accounts = self.accounts.lock().unwrap();
        accounts.upsert(renewed.clone());
        self.save_accounts(&accounts)?;
        Ok(renewed)
    }
}

fn stored_account(session: msa::MinecraftSession, refresh_token: String) -> MicrosoftAccount {
    MicrosoftAccount {
        uuid: session.uuid,
        name: session.name,
        xuid: session.xuid,
        skin_url: session.skin_url,
        refresh_token,
        access_token: session.access_token,
        expires_at: session.expires_at,
    }
}

fn auth_error(e: AuthError) -> CoreError {
    match e {
        AuthError::Cancelled => CoreError::Cancelled,
        AuthError::Network(e) => CoreError::Network(format!("network error during sign-in: {e}")),
        other => CoreError::Account(other.to_string()),
    }
}

/// A Microsoft sign-in in progress, polled by the UI.
#[derive(uniffi::Object)]
pub struct SignInTask {
    snapshot: Mutex<SignInSnapshot>,
    cancel: CancellationToken,
}

impl SignInTask {
    fn set(&self, update: impl FnOnce(&mut SignInSnapshot)) {
        update(&mut self.snapshot.lock().unwrap());
    }
}

#[uniffi::export]
impl SignInTask {
    pub fn snapshot(&self) -> SignInSnapshot {
        self.snapshot.lock().unwrap().clone()
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }
}

#[uniffi::export]
impl SulfurCore {
    /// Signed-in Microsoft accounts.
    pub fn accounts(&self) -> Vec<AccountInfo> {
        self.state.accounts.lock().unwrap().microsoft.iter().map(Into::into).collect()
    }

    /// The account the game starts with; `None` when there is none to use.
    pub fn current_account(&self) -> Option<AccountInfo> {
        self.state.current_account()
    }

    /// Forgets a Microsoft account and its tokens.
    pub fn remove_account(&self, id: String) -> Result<()> {
        let mut accounts = self.state.accounts.lock().unwrap();
        accounts.remove(&id);
        self.state.save_accounts(&accounts)
    }

    /// Starts signing in to a Microsoft account with a device code.
    pub fn sign_in_microsoft(&self) -> Arc<SignInTask> {
        let task = Arc::new(SignInTask {
            snapshot: Mutex::new(SignInSnapshot {
                stage: SignInStage::Starting,
                user_code: None,
                verification_uri: None,
                account: None,
                error: None,
            }),
            cancel: CancellationToken::new(),
        });
        let (state, job) = (self.state.clone(), task.clone());
        self.runtime.spawn(async move {
            match sign_in(&state, &job).await {
                Ok(account) => job.set(|s| {
                    s.stage = SignInStage::Done;
                    s.account = Some(account);
                }),
                Err(CoreError::Cancelled) => job.set(|s| s.stage = SignInStage::Cancelled),
                Err(e) => {
                    tracing::warn!(error = %e, "Microsoft sign-in failed");
                    job.set(|s| {
                        s.stage = SignInStage::Failed;
                        s.error = Some(e.to_string());
                    })
                }
            }
        });
        task
    }
}

async fn sign_in(state: &State, task: &SignInTask) -> Result<AccountInfo> {
    let client = state.msa()?;
    let code = tokio::select! {
        _ = task.cancel.cancelled() => return Err(CoreError::Cancelled),
        code = client.request_device_code() => code.map_err(auth_error)?,
    };
    task.set(|s| {
        s.stage = SignInStage::WaitingForPlayer;
        s.user_code = Some(code.user_code.clone());
        s.verification_uri = Some(code.verification_uri.clone());
    });
    let tokens = client.poll_device_code(&code, &task.cancel).await.map_err(auth_error)?;
    let session = client
        .minecraft_session(&tokens.access_token, |step| {
            task.set(|s| {
                s.stage = match step {
                    Step::Xbox => SignInStage::Xbox,
                    Step::Minecraft => SignInStage::Minecraft,
                    Step::Profile => SignInStage::Profile,
                }
            })
        })
        .await
        .map_err(auth_error)?;
    let account = stored_account(session, tokens.refresh_token);
    let info = AccountInfo::from(&account);
    let mut accounts = state.accounts.lock().unwrap();
    accounts.upsert(account);
    state.save_accounts(&accounts)?;
    Ok(info)
}
