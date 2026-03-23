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

// -- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{extract_codes, extract_links};

    // ── extract_codes ──────────────────────────────────────────────────────

    #[test]
    fn codes_empty_input() {
        assert!(extract_codes("").is_empty());
    }

    #[test]
    fn codes_no_digits() {
        assert!(extract_codes("hello world").is_empty());
    }

    #[test]
    fn codes_three_digits_below_min() {
        // 3 digits is too short — must not be extracted
        assert!(extract_codes("123").is_empty());
    }

    #[test]
    fn codes_four_digits_min_boundary() {
        assert_eq!(extract_codes("1234"), vec!["1234"]);
    }

    #[test]
    fn codes_eight_digits_max_boundary() {
        assert_eq!(extract_codes("12345678"), vec!["12345678"]);
    }

    #[test]
    fn codes_nine_digits_above_max() {
        // 9 digits is too long — must not be extracted
        assert!(extract_codes("123456789").is_empty());
    }

    #[test]
    fn codes_multiple_matches() {
        let result = extract_codes("pin: 4321 and token: 876543");
        assert_eq!(result, vec!["4321", "876543"]);
    }

    #[test]
    fn codes_surrounded_by_non_digit_chars() {
        let result = extract_codes("code=482910.");
        assert_eq!(result, vec!["482910"]);
    }

    #[test]
    fn codes_long_number_excluded_valid_neighbors_kept() {
        // 123456789 is 9 digits (skipped); flanking 4-digit codes are valid
        let result = extract_codes("1234 123456789 5678");
        assert_eq!(result, vec!["1234", "5678"]);
    }

    // ── extract_links ──────────────────────────────────────────────────────

    #[test]
    fn links_empty_input() {
        assert!(extract_links("").is_empty());
    }

    #[test]
    fn links_no_urls() {
        assert!(extract_links("hello world, no links here").is_empty());
    }

    #[test]
    fn links_single_https() {
        let result = extract_links("visit https://example.com now");
        assert_eq!(result, vec!["https://example.com"]);
    }

    #[test]
    fn links_single_http() {
        let result = extract_links("see http://example.com/path");
        assert_eq!(result, vec!["http://example.com/path"]);
    }

    #[test]
    fn links_multiple_urls() {
        let result = extract_links("a https://one.com b https://two.com/x c");
        assert_eq!(result, vec!["https://one.com", "https://two.com/x"]);
    }

    #[test]
    fn links_trailing_punctuation_stripped() {
        // Trailing '.' is not alphanumeric/'/'/':' so trim_matches removes it
        let result = extract_links("https://example.com.");
        assert_eq!(result, vec!["https://example.com"]);
    }

    #[test]
    fn links_ftp_not_extracted() {
        // Only http:// and https:// schemes are extracted
        assert!(extract_links("ftp://example.com").is_empty());
    }
}
