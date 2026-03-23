use crate::{
    auth::refresh,
    commands::{
        read::{fetch_message, AttachmentInfo},
        send::load_credentials,
    },
    errors::AvisError,
    output,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone)]
pub(crate) struct SavedFile {
    pub(crate) filename: String,
    pub(crate) path: String,
    pub(crate) size: u64,
}

#[derive(Serialize)]
struct DownloadResult {
    schema_version: &'static str,
    message_id: String,
    downloaded: Vec<SavedFile>,
}

/// Download attachments for a single message. Shared by `download`, `read`, and `wait`.
pub(crate) async fn download_attachments(
    client: &reqwest::Client,
    access_token: &str,
    message_id: &str,
    attachments: &[AttachmentInfo],
    dir: &Path,
) -> Result<Vec<SavedFile>, AvisError> {
    if attachments.is_empty() {
        return Ok(vec![]);
    }

    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| {
            AvisError::fs_error(format!("Cannot create dir {}: {}", dir.display(), e))
        })?;
    }

    let mut saved = Vec::new();

    for att in attachments {
        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}/attachments/{}",
            message_id, att.attachment_id
        );

        let resp = client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AvisError::imap_failure(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(AvisError::imap_failure(format!(
                "Failed to download {}: HTTP {}",
                att.filename,
                resp.status()
            )));
        }

        #[derive(serde::Deserialize)]
        struct AttachmentData {
            data: String,
        }

        let att_data: AttachmentData = resp
            .json()
            .await
            .map_err(|e| AvisError::imap_failure(e.to_string()))?;

        let bytes = decode_attachment_data(&att_data.data)
            .map_err(|e| AvisError::new("decode_error", format!("{}: {}", att.filename, e)))?;

        let file_path = dir.join(&att.filename);
        std::fs::write(&file_path, &bytes)
            .map_err(|e| AvisError::fs_error(format!("Cannot write {}: {}", att.filename, e)))?;

        saved.push(SavedFile {
            filename: att.filename.clone(),
            path: file_path.to_string_lossy().to_string(),
            size: bytes.len() as u64,
        });
    }

    Ok(saved)
}

pub async fn run(home: &Path, identity: &str, message_id: Option<&str>, dir: &str) {
    let creds = load_credentials(home, identity).unwrap_or_else(|e| e.bail(2));

    let token = refresh::get_access_token(&creds)
        .await
        .unwrap_or_else(|e| e.bail(2));

    let client = reqwest::Client::new();

    // Resolve message ID
    let target_id = match message_id {
        Some(id) => id.to_string(),
        None => resolve_latest_message_id(&client, &token.access_token).await,
    };

    // Fetch message to get attachment metadata
    let msg = fetch_message(&client, &token.access_token, &target_id)
        .await
        .unwrap_or_else(|e| e.bail(2));

    if msg.attachments.is_empty() {
        AvisError::new("no_attachments", "Message has no attachments").bail(1);
    }

    let downloaded = download_attachments(
        &client,
        &token.access_token,
        &target_id,
        &msg.attachments,
        Path::new(dir),
    )
    .await
    .unwrap_or_else(|e| e.bail(2));

    output::print_json(&DownloadResult {
        schema_version: output::SCHEMA_VERSION,
        message_id: target_id,
        downloaded,
    });
}

fn decode_attachment_data(data: &str) -> Result<Vec<u8>, String> {
    if let Ok(bytes) = URL_SAFE_NO_PAD.decode(data) {
        return Ok(bytes);
    }

    let normalized = data.replace('-', "+").replace('_', "/");
    let padded = match normalized.len() % 4 {
        2 => format!("{}==", normalized),
        3 => format!("{}=", normalized),
        _ => normalized,
    };

    base64::engine::general_purpose::STANDARD
        .decode(&padded)
        .map_err(|e| format!("base64 decode failed: {}", e))
}

async fn resolve_latest_message_id(client: &reqwest::Client, access_token: &str) -> String {
    let url = "https://gmail.googleapis.com/gmail/v1/users/me/messages?q=in:inbox&maxResults=1";

    #[derive(serde::Deserialize)]
    struct ListResponse {
        messages: Option<Vec<MessageRef>>,
    }

    #[derive(serde::Deserialize)]
    struct MessageRef {
        id: String,
    }

    let resp = client
        .get(url)
        .bearer_auth(access_token)
        .send()
        .await
        .unwrap_or_else(|e| AvisError::imap_failure(e.to_string()).bail(2));

    let list: ListResponse = resp
        .json()
        .await
        .unwrap_or_else(|e| AvisError::imap_failure(e.to_string()).bail(2));

    list.messages
        .and_then(|m| m.into_iter().next())
        .map(|m| m.id)
        .unwrap_or_else(|| AvisError::new("empty_inbox", "No messages found in inbox").bail(1))
}
