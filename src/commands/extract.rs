use crate::{
    auth::refresh,
    commands::{read::fetch_message, send::load_credentials},
    errors::AvisError,
    output,
};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct ExtractResult {
    schema_version: &'static str,
    message_id: String,
    codes: Vec<String>,
    links: Vec<String>,
}

pub async fn run(
    home: &Path,
    identity: &str,
    message_id: Option<&str>,
    codes: bool,
    links: bool,
    first_code: bool,
    first_link: bool,
) {
    // Exactly one selector required
    let selector_count = [codes, links, first_code, first_link]
        .iter()
        .filter(|&&x| x)
        .count();

    if selector_count == 0 {
        AvisError::new(
            "missing_selector",
            "Provide exactly one of: --codes, --links, --first-code, --first-link",
        )
        .bail(1);
    }

    let creds = load_credentials(home, identity).unwrap_or_else(|e| e.bail(2));

    let token = refresh::get_access_token(&creds)
        .await
        .unwrap_or_else(|e| e.bail(2));

    let client = reqwest::Client::new();

    // Resolve which message to extract from
    let target_id = match message_id {
        Some(id) => id.to_string(),
        None => {
            // Get latest inbox message ID
            let url =
                "https://gmail.googleapis.com/gmail/v1/users/me/messages?q=in:inbox&maxResults=1";

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
                .bearer_auth(&token.access_token)
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
                .unwrap_or_else(|| {
                    AvisError::new("empty_inbox", "No messages found in inbox").bail(1)
                })
        }
    };

    // Fetch the message
    let msg = fetch_message(&client, &token.access_token, &target_id)
        .await
        .unwrap_or_else(|e| e.bail(2));

    // Extract codes and links from body
    let all_codes = extract_codes(&msg.body);
    let all_links = extract_links(&msg.body);

    let (out_codes, out_links) = if first_code {
        (all_codes.into_iter().take(1).collect(), vec![])
    } else if first_link {
        (vec![], all_links.into_iter().take(1).collect())
    } else if codes {
        (all_codes, vec![])
    } else {
        (vec![], all_links)
    };

    output::print_json(&ExtractResult {
        schema_version: output::SCHEMA_VERSION,
        message_id: target_id,
        codes: out_codes,
        links: out_links,
    });
}

// -- Extractors ------------------------------------------------------------

/// Extract numeric codes 4-8 digits long from text
fn extract_codes(text: &str) -> Vec<String> {
    let mut codes = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();

    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let len = i - start;
            if (4..=8).contains(&len) {
                // Make sure it's not part of a longer number
                let before_ok = start == 0 || !chars[start - 1].is_ascii_digit();
                let after_ok = i >= chars.len() || !chars[i].is_ascii_digit();
                if before_ok && after_ok {
                    codes.push(chars[start..i].iter().collect());
                }
            }
        } else {
            i += 1;
        }
    }
    codes
}

/// Extract URLs from text
fn extract_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();

    for word in text.split_whitespace() {
        let word = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != ':');
        if word.starts_with("http://") || word.starts_with("https://") {
            links.push(word.to_string());
        }
    }

    links
}
