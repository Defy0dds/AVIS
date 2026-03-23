use crate::{
    auth::{pkce::PkceChallenge, refresh},
    config::{self, IdentityConfig},
    crypto,
    errors::AvisError,
    output,
};
use serde::Serialize;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// -- Bundled OAuth2 client credentials -------------------------------------
// Replace these with your actual GCP client ID and secret from the
// downloaded client_secret_xxxx.json file.
const CLIENT_ID: &str = "205426579886-cg8egldc3kkp8cnlp00v5qsm1qee9am9.apps.googleusercontent.com";
const CLIENT_SECRET: &str = "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";

const SCOPES: &str = "https://www.googleapis.com/auth/gmail.send \
     https://www.googleapis.com/auth/gmail.readonly";

// -- add -------------------------------------------------------------------

#[derive(Serialize)]
struct AddResult {
    schema_version: &'static str,
    identity: String,
    email: String,
    status: &'static str,
}

pub async fn add(home: &Path, name: &str, email: &str) {
    // 1. Check identity doesn't already exist
    let id_dir = config::identity_dir(home, name);
    if id_dir.exists() {
        AvisError::identity_exists(name).bail(1);
    }

    // 2. Create identity directory
    std::fs::create_dir_all(&id_dir).unwrap_or_else(|e| AvisError::fs_error(e.to_string()).bail(2));

    // 3. Generate PKCE challenge
    let pkce = PkceChallenge::new();

    // 4. Find a free loopback port (up to 5 attempts)
    let (listener, port) = bind_loopback_port().await.unwrap_or_else(|e| e.bail(2));

    let redirect_uri = format!("http://127.0.0.1:{}", port);

    // 5. Build Google OAuth2 authorization URL
    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth\
         ?client_id={}\
         &redirect_uri={}\
         &response_type=code\
         &scope={}\
         &code_challenge={}\
         &code_challenge_method=S256\
         &access_type=offline\
         &prompt=consent",
        urlencoding::encode(CLIENT_ID),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(SCOPES),
        urlencoding::encode(&pkce.code_challenge),
    );

    // 6. Open browser
    eprintln!("Opening browser for Google authentication...");
    eprintln!("If the browser does not open, visit:\n{}", auth_url);

    if open::that(&auth_url).is_err() {
        eprintln!("Could not open browser automatically. Please open the URL above manually.");
    }

    // 7. Wait for redirect with auth code
    eprintln!("Waiting for Google to redirect back...");
    let code = wait_for_auth_code(listener)
        .await
        .unwrap_or_else(|e| e.bail(2));

    // 8. Exchange code for refresh token
    let creds = refresh::exchange_code(
        &code,
        &pkce.code_verifier,
        CLIENT_ID,
        CLIENT_SECRET,
        &redirect_uri,
    )
    .await
    .unwrap_or_else(|e| e.bail(2));

    // 9. Generate master key and encrypt credentials
    let key_path = config::identity_master_key_path(home, name);
    let key = crypto::generate_master_key(&key_path).unwrap_or_else(|e| e.bail(2));

    let creds_json =
        serde_json::to_vec(&creds).unwrap_or_else(|e| AvisError::fs_error(e.to_string()).bail(2));

    let encrypted = crypto::encrypt(&key, &creds_json).unwrap_or_else(|e| e.bail(2));

    let creds_path = config::identity_credentials_path(home, name);
    std::fs::write(&creds_path, &encrypted)
        .unwrap_or_else(|e| AvisError::fs_error(e.to_string()).bail(2));

    // 10. Write config.json
    let identity_config = IdentityConfig::new(name, email);
    let config_json = serde_json::to_string_pretty(&identity_config)
        .unwrap_or_else(|e| AvisError::fs_error(e.to_string()).bail(2));

    let config_path = config::identity_config_path(home, name);
    std::fs::write(&config_path, config_json)
        .unwrap_or_else(|e| AvisError::fs_error(e.to_string()).bail(2));

    output::print_json(&AddResult {
        schema_version: output::SCHEMA_VERSION,
        identity: name.to_string(),
        email: email.to_string(),
        status: "ready",
    });
}

// -- list ------------------------------------------------------------------

#[derive(Serialize)]
struct ListResult {
    schema_version: &'static str,
    identities: Vec<IdentityEntry>,
}

#[derive(Serialize)]
struct IdentityEntry {
    name: String,
    email: String,
}

pub async fn list(home: &Path) {
    let ids_dir = config::identities_dir(home);

    let mut identities = Vec::new();

    if ids_dir.exists() {
        let entries = std::fs::read_dir(&ids_dir)
            .unwrap_or_else(|e| AvisError::fs_error(e.to_string()).bail(2));

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let email = config::load_identity(home, &name)
                .map(|c| c.email)
                .unwrap_or_else(|_| "-".to_string());
            identities.push(IdentityEntry { name, email });
        }
    }

    output::print_json(&ListResult {
        schema_version: output::SCHEMA_VERSION,
        identities,
    });
}

// -- show ------------------------------------------------------------------

#[derive(Serialize)]
struct ShowResult {
    schema_version: &'static str,
    name: String,
    email: String,
    provider: String,
    status: &'static str,
}

pub async fn show(home: &Path, name: &str) {
    let cfg = config::load_identity(home, name).unwrap_or_else(|e| e.bail(1));

    output::print_json(&ShowResult {
        schema_version: output::SCHEMA_VERSION,
        name: cfg.name,
        email: cfg.email,
        provider: cfg.provider,
        status: "ready",
    });
}

// -- remove ----------------------------------------------------------------

#[derive(Serialize)]
struct RemoveResult {
    schema_version: &'static str,
    deleted: bool,
}

pub async fn remove(home: &Path, name: &str) {
    // Confirm identity exists first
    config::load_identity(home, name).unwrap_or_else(|e| e.bail(1));

    // Prompt
    eprint!("Delete identity '{}'? [y/N]: ", name);
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .unwrap_or_else(|e| AvisError::fs_error(e.to_string()).bail(2));

    if input.trim().to_lowercase() != "y" {
        output::print_json(&RemoveResult {
            schema_version: output::SCHEMA_VERSION,
            deleted: false,
        });
        return;
    }

    let id_dir = config::identity_dir(home, name);
    std::fs::remove_dir_all(&id_dir).unwrap_or_else(|e| AvisError::fs_error(e.to_string()).bail(2));

    output::print_json(&RemoveResult {
        schema_version: output::SCHEMA_VERSION,
        deleted: true,
    });
}

// -- Helpers ---------------------------------------------------------------

/// Try to bind a loopback TCP listener on a random port. Retries up to 5 times.
async fn bind_loopback_port() -> Result<(tokio::net::TcpListener, u16), AvisError> {
    for _ in 0..5 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await;
        if let Ok(l) = listener {
            let port = l
                .local_addr()
                .map_err(|e| AvisError::oauth_failed(e.to_string()))?
                .port();
            return Ok((l, port));
        }
    }
    Err(AvisError::oauth_failed(
        "Could not bind loopback port after 5 attempts",
    ))
}

/// Accept one HTTP request on the listener and extract the `code` query param.
async fn wait_for_auth_code(listener: tokio::net::TcpListener) -> Result<String, AvisError> {
    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|e| AvisError::oauth_failed(e.to_string()))?;

    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| AvisError::oauth_failed(e.to_string()))?;

    let request = String::from_utf8_lossy(&buf[..n]);

    // Send a response so the browser doesn't hang
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
        <html><body><h2>AVIS: Authentication complete.</h2>\
        <p>You can close this tab and return to your terminal.</p>\
        </body></html>";

    stream.write_all(response.as_bytes()).await.ok();

    // Parse code from: GET /?code=xxxx&... HTTP/1.1
    let code = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|path| {
            path.split('?').nth(1).and_then(|query| {
                query.split('&').find_map(|param| {
                    let (key, val) = param.split_once('=')?;
                    if key == "code" {
                        Some(val.to_string())
                    } else {
                        None
                    }
                })
            })
        })
        .ok_or_else(|| AvisError::oauth_failed("No auth code in Google redirect"))?;

    Ok(code)
}
