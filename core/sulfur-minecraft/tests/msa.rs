//! The Microsoft sign-in chain against a local stand-in for every service.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use sulfur_download::{CancellationToken, DownloadConfig, Downloader, MirrorSet};
use sulfur_minecraft::msa::{AuthError, Endpoints, MsaClient, Step};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[derive(Clone, Copy)]
enum Account {
    Owner,
    NoProfile,
    Child,
}

async fn serve(account: Account) -> (Endpoints, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let polls = Arc::new(AtomicUsize::new(0));
    let counter = polls.clone();
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            tokio::spawn(handle(stream, account, counter.clone()));
        }
    });
    let endpoints = Endpoints {
        device_code: format!("{base}/devicecode"),
        token: format!("{base}/token"),
        xbox_user: format!("{base}/xbl"),
        xsts: format!("{base}/xsts"),
        minecraft_login: format!("{base}/login"),
        profile: format!("{base}/profile"),
    };
    (endpoints, polls)
}

async fn handle(mut stream: TcpStream, account: Account, polls: Arc<AtomicUsize>) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        match stream.read(&mut tmp).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let length: usize = head
        .lines()
        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap()))
        .unwrap_or(0);
    while buf.len() < head_end + length {
        match stream.read(&mut tmp).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
    }
    let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
    let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();

    let (status, json) = match path.as_str() {
        "/devicecode" => {
            assert!(body.contains("client_id=test-client"), "{body}");
            (200, r#"{"user_code":"ABCD-EFGH","device_code":"dev","verification_uri":"https://www.microsoft.com/link","expires_in":60,"interval":1}"#)
        }
        "/token" if polls.fetch_add(1, Ordering::SeqCst) == 0 => (400, r#"{"error":"authorization_pending"}"#),
        "/token" => {
            assert!(body.contains("device_code=dev"), "{body}");
            (200, r#"{"access_token":"msa","refresh_token":"refresh"}"#)
        }
        "/xbl" => {
            assert!(body.contains("d=msa"), "{body}");
            (200, r#"{"Token":"xbl","DisplayClaims":{"xui":[{"uhs":"hash"}]}}"#)
        }
        "/xsts" if matches!(account, Account::Child) => (401, r#"{"XErr":2148916238}"#),
        "/xsts" => {
            assert!(body.contains("\"xbl\""), "{body}");
            (200, r#"{"Token":"xsts","DisplayClaims":{"xui":[{"uhs":"hash","xid":"2535"}]}}"#)
        }
        "/login" => {
            assert!(body.contains("XBL3.0 x=hash;xsts"), "{body}");
            (200, r#"{"access_token":"mc","expires_in":86400}"#)
        }
        "/profile" if matches!(account, Account::NoProfile) => (404, r#"{"error":"NOT_FOUND"}"#),
        "/profile" => {
            assert!(head.contains("Bearer mc"), "{head}");
            (200, r#"{"id":"069a79f4-44e9-4726-a5be-fca90e38aaf5","name":"Notch","skins":[{"state":"ACTIVE","url":"http://textures.minecraft.net/texture/abc"}]}"#)
        }
        _ => (404, "{}"),
    };
    let reply = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    );
    let _ = stream.write_all(reply.as_bytes()).await;
}

fn client(endpoints: Endpoints) -> MsaClient {
    let http = Downloader::new(DownloadConfig::default(), MirrorSet::default()).unwrap().http_client();
    MsaClient::with_endpoints(http, "test-client", endpoints)
}

#[tokio::test]
async fn signs_in_with_a_device_code() {
    let (endpoints, polls) = serve(Account::Owner).await;
    let msa = client(endpoints);
    let code = msa.request_device_code().await.unwrap();
    assert_eq!(code.user_code, "ABCD-EFGH");
    let tokens = msa.poll_device_code(&code, &CancellationToken::new()).await.unwrap();
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    assert_eq!(tokens.refresh_token, "refresh");

    let mut steps = Vec::new();
    let session = msa.minecraft_session(&tokens.access_token, |s| steps.push(s)).await.unwrap();
    assert_eq!(steps, [Step::Xbox, Step::Minecraft, Step::Profile]);
    assert_eq!(session.uuid, "069a79f444e94726a5befca90e38aaf5");
    assert_eq!(session.name, "Notch");
    assert_eq!(session.xuid, "2535");
    assert_eq!(session.access_token, "mc");
    assert_eq!(session.skin_url.as_deref(), Some("https://textures.minecraft.net/texture/abc"));
}

#[tokio::test]
async fn reports_accounts_that_cannot_play() {
    let (endpoints, _) = serve(Account::NoProfile).await;
    let err = client(endpoints).minecraft_session("msa", |_| {}).await.unwrap_err();
    assert!(matches!(err, AuthError::NotOwned), "{err}");

    let (endpoints, _) = serve(Account::Child).await;
    let err = client(endpoints).minecraft_session("msa", |_| {}).await.unwrap_err();
    assert!(matches!(err, AuthError::ChildAccount), "{err}");
}

#[tokio::test]
async fn polling_stops_when_cancelled() {
    let (endpoints, _) = serve(Account::Owner).await;
    let msa = client(endpoints);
    let code = msa.request_device_code().await.unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(msa.poll_device_code(&code, &cancel).await, Err(AuthError::Cancelled)));
}
