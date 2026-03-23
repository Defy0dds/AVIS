use crate::{auth::refresh, config, crypto, errors::AvisError, output};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct SendResult {
    schema_version: &'static str,
    sent: bool,
    from: String,
    to: String,
    subject: String,
    message_id: String,
    ts: String,
}

#[derive(Serialize)]
struct SendFailure {
    schema_version: &'static str,
    sent: bool,
    error: String,
    message: String,
}

pub async fn run(
    home: &Path,
    identity: &str,
    to: &str,
    subject: &str,
    body: &str,
    attachments: &[String],
) {
    let cfg = config::load_identity(home, identity).unwrap_or_else(|e| e.bail(1));

    let creds = load_credentials(home, identity).unwrap_or_else(|e| e.bail(2));

    let token = refresh::get_access_token(&creds)
        .await
        .unwrap_or_else(|e| e.bail(2));

    // Build RFC 2822 raw email
    let message_id = format!(
        "<{}.{}@avis.local>",
        uuid_simple(),
        cfg.email.replace('@', ".at.")
    );

    let raw_email = if attachments.is_empty() {
        format!(
            "From: {}\r\nTo: {}\r\nSubject: {}\r\nMessage-ID: {}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}",
            cfg.email, to, subject, message_id, body
        )
    } else {
        build_multipart_email(&cfg.email, to, subject, &message_id, body, attachments)
            .unwrap_or_else(|e| e.bail(1))
    };

    // Gmail API expects base64url encoded raw email
    let encoded = URL_SAFE_NO_PAD.encode(raw_email.as_bytes());

    let client = reqwest::Client::new();

    let mut last_err = String::new();
    let mut sent = false;
    let delays = [1u64, 2, 4];

    for (attempt, &delay) in delays.iter().enumerate() {
        let resp = client
            .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
            .bearer_auth(&token.access_token)
            .json(&serde_json::json!({ "raw": encoded }))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                sent = true;
                break;
            }
            Ok(r) => {
                last_err = format!(
                    "HTTP {}: {}",
                    r.status(),
                    r.text().await.unwrap_or_default()
                );
                if attempt < 2 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                }
            }
            Err(e) => {
                last_err = e.to_string();
                if attempt < 2 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                }
            }
        }
    }

    if !sent {
        output::print_json(&SendFailure {
            schema_version: output::SCHEMA_VERSION,
            sent: false,
            error: "smtp_failure".to_string(),
            message: last_err,
        });
        std::process::exit(2);
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format_ts(d.as_secs()))
        .unwrap_or_else(|_| "unknown".to_string());

    output::print_json(&SendResult {
        schema_version: output::SCHEMA_VERSION,
        sent: true,
        from: cfg.email,
        to: to.to_string(),
        subject: subject.to_string(),
        message_id,
        ts,
    });
}

// -- MIME multipart --------------------------------------------------------

fn build_multipart_email(
    from: &str,
    to: &str,
    subject: &str,
    message_id: &str,
    body: &str,
    attachments: &[String],
) -> Result<String, AvisError> {
    use base64::engine::general_purpose::STANDARD;

    let boundary = format!("avis-{}", uuid_simple());

    let mut email = String::new();
    email.push_str(&format!("From: {}\r\n", from));
    email.push_str(&format!("To: {}\r\n", to));
    email.push_str(&format!("Subject: {}\r\n", subject));
    email.push_str(&format!("Message-ID: {}\r\n", message_id));
    email.push_str("MIME-Version: 1.0\r\n");
    email.push_str(&format!(
        "Content-Type: multipart/mixed; boundary=\"{}\"\r\n",
        boundary
    ));
    email.push_str("\r\n");

    // Text body part
    email.push_str(&format!("--{}\r\n", boundary));
    email.push_str("Content-Type: text/plain; charset=utf-8\r\n");
    email.push_str("\r\n");
    email.push_str(body);
    email.push_str("\r\n");

    // Attachment parts
    for path_str in attachments {
        let path = std::path::Path::new(path_str);

        if !path.exists() {
            return Err(AvisError::new(
                "attachment_not_found",
                format!("File not found: {}", path_str),
            ));
        }

        let file_bytes = std::fs::read(path)
            .map_err(|e| AvisError::new("attachment_read_error", format!("{}: {}", path_str, e)))?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("attachment");

        let mime = guess_mime_type(filename);
        let b64 = STANDARD.encode(&file_bytes);

        email.push_str(&format!("--{}\r\n", boundary));
        email.push_str(&format!(
            "Content-Type: {}; name=\"{}\"\r\n",
            mime, filename
        ));
        email.push_str("Content-Transfer-Encoding: base64\r\n");
        email.push_str(&format!(
            "Content-Disposition: attachment; filename=\"{}\"\r\n",
            filename
        ));
        email.push_str("\r\n");

        // Wrap base64 at 76 chars per RFC 2045
        for chunk in b64.as_bytes().chunks(76) {
            email.push_str(std::str::from_utf8(chunk).unwrap_or_default());
            email.push_str("\r\n");
        }
    }

    // Closing boundary
    email.push_str(&format!("--{}--\r\n", boundary));

    Ok(email)
}

fn guess_mime_type(filename: &str) -> &'static str {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "gz" | "gzip" => "application/gzip",
        "tar" => "application/x-tar",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

// -- Helpers ---------------------------------------------------------------

pub fn load_credentials(
    home: &Path,
    identity: &str,
) -> Result<refresh::OAuthCredentials, AvisError> {
    let key_path = config::identity_master_key_path(home, identity);
    let creds_path = config::identity_credentials_path(home, identity);

    let key = crypto::load_master_key(&key_path)?;
    let encrypted = std::fs::read(&creds_path).map_err(|e| AvisError::fs_error(e.to_string()))?;
    let decrypted = crypto::decrypt(&key, &encrypted)?;
    serde_json::from_slice(&decrypted).map_err(|_| AvisError::credentials_corrupt(identity))
}

/// Generate a simple unique ID without pulling in a uuid crate
fn uuid_simple() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", t)
}

fn format_ts(secs: u64) -> String {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut y = 1970u64;
    loop {
        let leap = is_leap(y);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 1u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        mo += 1;
    }
    (y, mo, days + 1)
}

fn is_leap(y: u64) -> bool {
    y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}
