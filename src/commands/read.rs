use crate::{
    auth::refresh,
    commands::{download::SavedFile, send::load_credentials},
    errors::AvisError,
    output, sanitize,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize)]
struct ReadResult {
    schema_version: &'static str,
    messages: Vec<EmailMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    downloaded_files: Vec<SavedFile>,
}

#[derive(Serialize)]
pub(crate) struct EmailMessage {
    pub(crate) id: String,
    pub(crate) from: String,
    pub(crate) subject: String,
    pub(crate) body: String,
    pub(crate) ts: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) attachments: Vec<AttachmentInfo>,
}

#[derive(Serialize, Clone)]
pub(crate) struct AttachmentInfo {
    pub(crate) filename: String,
    pub(crate) mime_type: String,
    pub(crate) size: u64,
    #[serde(skip_serializing)]
    pub(crate) attachment_id: String,
}

#[derive(Deserialize)]
struct ListResponse {
    messages: Option<Vec<MessageRef>>,
}

#[derive(Deserialize)]
struct MessageRef {
    id: String,
}

#[derive(Deserialize)]
struct GmailMessage {
    id: String,
    payload: Option<Payload>,
    #[serde(rename = "internalDate")]
    internal_date: Option<String>,
}

#[derive(Deserialize)]
struct Payload {
    headers: Option<Vec<Header>>,
    body: Option<Body>,
    parts: Option<Vec<Part>>,
}

#[derive(Deserialize)]
struct Header {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct Body {
    data: Option<String>,
    #[serde(rename = "attachmentId")]
    attachment_id: Option<String>,
    size: Option<u64>,
}

#[derive(Deserialize)]
struct Part {
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    filename: Option<String>,
    body: Option<Body>,
    parts: Option<Vec<Part>>,
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    home: &Path,
    identity: &str,
    latest: bool,
    from_filter: Option<&str>,
    subject_filter: Option<&str>,
    count: usize,
    _verbose: bool,
    download_dir: Option<&str>,
) {
    let creds = load_credentials(home, identity).unwrap_or_else(|e| e.bail(2));

    let token = refresh::get_access_token(&creds)
        .await
        .unwrap_or_else(|e| e.bail(2));

    let client = reqwest::Client::new();

    // Build query string for Gmail API
    let mut query_parts = vec!["in:inbox".to_string()];
    if let Some(f) = from_filter {
        query_parts.push(format!("from:{}", f));
    }
    if let Some(s) = subject_filter {
        query_parts.push(format!("subject:{}", s));
    }
    let query = query_parts.join(" ");

    let max_results = if latest { 1 } else { count };

    let list_url = format!(
        "https://gmail.googleapis.com/gmail/v1/users/me/messages?q={}&maxResults={}",
        urlencoding::encode(&query),
        max_results
    );

    let list_resp = client
        .get(&list_url)
        .bearer_auth(&token.access_token)
        .send()
        .await
        .unwrap_or_else(|e| AvisError::imap_failure(e.to_string()).bail(2));

    let list: ListResponse = list_resp
        .json()
        .await
        .unwrap_or_else(|e| AvisError::imap_failure(e.to_string()).bail(2));

    let message_refs = list.messages.unwrap_or_default();

    // Fetch each message
    let mut messages = Vec::new();

    for msg_ref in message_refs {
        let msg = fetch_message(&client, &token.access_token, &msg_ref.id)
            .await
            .unwrap_or_else(|e| e.bail(2));
        messages.push(msg);
    }

    // Auto-download attachments if requested
    let mut downloaded_files = Vec::new();
    if let Some(dir) = download_dir {
        let dir_path = Path::new(dir);
        for msg in &messages {
            if !msg.attachments.is_empty() {
                let saved = crate::commands::download::download_attachments(
                    &client,
                    &token.access_token,
                    &msg.id,
                    &msg.attachments,
                    dir_path,
                )
                .await
                .unwrap_or_else(|e| e.bail(2));
                downloaded_files.extend(saved);
            }
        }
    }

    output::print_json(&ReadResult {
        schema_version: output::SCHEMA_VERSION,
        messages,
        downloaded_files,
    });
}

// -- Fetch single message --------------------------------------------------

pub async fn fetch_message(
    client: &reqwest::Client,
    access_token: &str,
    message_id: &str,
) -> Result<EmailMessage, AvisError> {
    let msg_url = format!(
        "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}?format=full",
        message_id
    );

    let resp = client
        .get(msg_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| AvisError::imap_failure(e.to_string()))?;

    let msg: GmailMessage = resp
        .json()
        .await
        .map_err(|e| AvisError::imap_failure(e.to_string()))?;

    let payload = msg.payload.unwrap_or(Payload {
        headers: None,
        body: None,
        parts: None,
    });

    // Extract headers
    let headers = payload.headers.unwrap_or_default();
    let from = headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("from"))
        .map(|h| h.value.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let subject = headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("subject"))
        .map(|h| h.value.clone())
        .unwrap_or_else(|| "(no subject)".to_string());

    // Extract body - prefer plain text part, fall back to body.data
    let raw_body = extract_plain_text(&payload.body, &payload.parts);

    // Extract attachment metadata
    let attachments = extract_attachment_info(&payload.parts);

    // Clean body: strip HTML, remove quoted lines, trim, cap at 2000 chars
    let clean_body = clean_body(&raw_body);

    // Timestamp from internalDate (milliseconds since epoch)
    let ts = msg
        .internal_date
        .as_deref()
        .and_then(|d| d.parse::<u64>().ok())
        .map(|ms| format_ts(ms / 1000))
        .unwrap_or_else(|| "unknown".to_string());

    Ok(EmailMessage {
        id: msg.id,
        from,
        subject,
        body: clean_body,
        ts,
        attachments,
    })
}

fn extract_plain_text(body: &Option<Body>, parts: &Option<Vec<Part>>) -> String {
    // Try multipart parts first
    if let Some(parts) = parts {
        for part in parts {
            if part.mime_type.as_deref() == Some("text/plain") {
                if let Some(b) = &part.body {
                    if let Some(data) = &b.data {
                        return decode_base64url(data);
                    }
                }
            }
        }
    }

    // Fall back to top-level body
    if let Some(b) = body {
        if let Some(data) = &b.data {
            return decode_base64url(data);
        }
    }

    String::new()
}

fn extract_attachment_info(parts: &Option<Vec<Part>>) -> Vec<AttachmentInfo> {
    let mut attachments = Vec::new();
    if let Some(parts) = parts {
        collect_attachments(parts, &mut attachments);
    }
    attachments
}

fn collect_attachments(parts: &[Part], out: &mut Vec<AttachmentInfo>) {
    for part in parts {
        // A part is an attachment if it has a non-empty filename and an attachmentId
        if let Some(filename) = &part.filename {
            if !filename.is_empty() {
                if let Some(body) = &part.body {
                    if let Some(att_id) = &body.attachment_id {
                        // Fix #7: sanitize filename before including in JSON output
                        let safe_name = match sanitize::sanitize_filename(filename) {
                            Ok(n) => n,
                            Err(_) => continue, // skip attachments with invalid filenames
                        };
                        out.push(AttachmentInfo {
                            filename: safe_name,
                            mime_type: part
                                .mime_type
                                .clone()
                                .unwrap_or_else(|| "application/octet-stream".to_string()),
                            size: body.size.unwrap_or(0),
                            attachment_id: att_id.clone(),
                        });
                    }
                }
            }
        }
        // Recurse into nested parts (e.g. multipart/mixed inside multipart/alternative)
        if let Some(sub_parts) = &part.parts {
            collect_attachments(sub_parts, out);
        }
    }
}

fn decode_base64url(data: &str) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    // Normalize: replace URL-safe chars, then try with and without padding
    let normalized = data.replace('-', "+").replace('_', "/");

    // Try URL_SAFE_NO_PAD first
    if let Ok(bytes) = URL_SAFE_NO_PAD.decode(data.replace('-', "+").replace('_', "/")) {
        if let Ok(s) = String::from_utf8(bytes) {
            return s;
        }
    }

    // Try adding padding
    let padded = match normalized.len() % 4 {
        2 => format!("{}==", normalized),
        3 => format!("{}=", normalized),
        _ => normalized,
    };

    base64::engine::general_purpose::STANDARD
        .decode(&padded)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_default()
}

fn clean_body(raw: &str) -> String {
    let mut lines: Vec<&str> = raw
        .lines()
        .filter(|l| !l.starts_with('>'))
        .filter(|l| !l.trim_start().starts_with("On ") || !l.contains("wrote:"))
        .collect();

    // Trim trailing blank lines
    while lines
        .last()
        .map(|l: &&str| l.trim().is_empty())
        .unwrap_or(false)
    {
        lines.pop();
    }

    let result = lines.join("\n").trim().to_string();

    // Fix #6: strip null bytes and non-printable control chars (keep \n, \r, \t)
    let result = sanitize::strip_control_chars(&result);

    // Cap at 2000 chars
    if result.len() > 2000 {
        format!("{}...[truncated]", &result[..2000])
    } else {
        result
    }
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
        let diy = if leap { 366 } else { 365 };
        if days < diy {
            break;
        }
        days -= diy;
        y += 1;
    }
    let leap = is_leap(y);
    let md = [
        31u64,
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
    for &d in &md {
        if days < d {
            break;
        }
        days -= d;
        mo += 1;
    }
    (y, mo, days + 1)
}

fn is_leap(y: u64) -> bool {
    y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}

// -- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::clean_body;

    #[test]
    fn clean_empty_input() {
        assert_eq!(clean_body(""), "");
    }

    #[test]
    fn clean_no_special_lines() {
        assert_eq!(clean_body("Hello\nWorld"), "Hello\nWorld");
    }

    #[test]
    fn clean_strips_quoted_lines() {
        let input = "> this is a quote\nActual content";
        assert_eq!(clean_body(input), "Actual content");
    }

    #[test]
    fn clean_strips_multiple_quoted_lines() {
        let input = "> line one\n> line two\nReal content\n> line three";
        assert_eq!(clean_body(input), "Real content");
    }

    #[test]
    fn clean_strips_on_wrote_line() {
        // Lines that both start with "On " and contain "wrote:" are removed
        let input = "On Mon, 1 Jan 2024, Bob <b@b.com> wrote:\n> quoted\nActual";
        assert_eq!(clean_body(input), "Actual");
    }

    #[test]
    fn clean_keeps_on_line_without_wrote() {
        // "On" without "wrote:" must be preserved
        let input = "On the other hand, it works.\nMore content";
        assert_eq!(
            clean_body(input),
            "On the other hand, it works.\nMore content"
        );
    }

    #[test]
    fn clean_strips_indented_on_wrote_line() {
        // trim_start means leading spaces don't protect an On…wrote: line
        let input = "  On Monday Alice wrote:\nReply body";
        assert_eq!(clean_body(input), "Reply body");
    }

    #[test]
    fn clean_strips_trailing_blank_lines() {
        let input = "Content\n\n\n";
        assert_eq!(clean_body(input), "Content");
    }

    #[test]
    fn clean_truncates_above_2000_chars() {
        let long = "a".repeat(2001);
        let result = clean_body(&long);
        assert!(result.starts_with(&"a".repeat(2000)));
        assert!(result.ends_with("...[truncated]"));
    }

    #[test]
    fn clean_no_truncation_at_exactly_2000_chars() {
        let exactly = "a".repeat(2000);
        let result = clean_body(&exactly);
        assert_eq!(result, exactly);
        assert!(!result.contains("...[truncated]"));
    }
}
