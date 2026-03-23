use crate::{
    auth::refresh,
    commands::{read::fetch_message, send::load_credentials},
    output,
};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct WaitTimeout {
    schema_version: &'static str,
    matched: bool,
    timeout: u64,
}

pub async fn run(
    home: &Path,
    identity: &str,
    from_filter: Option<&str>,
    subject_filter: Option<&str>,
    timeout: u64,
) {
    let creds = load_credentials(home, identity).unwrap_or_else(|e| e.bail(2));

    let client = reqwest::Client::new();

    // Build Gmail query
    let mut query_parts = vec!["in:inbox".to_string()];
    if let Some(f) = from_filter {
        query_parts.push(format!("from:{}", f));
    }
    if let Some(s) = subject_filter {
        query_parts.push(format!("subject:{}", s));
    }
    let query = urlencoding::encode(&query_parts.join(" ")).to_string();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout);

    // Track seen message IDs so we only match NEW messages
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Seed seen_ids with messages already in inbox before we start waiting
    let token = refresh::get_access_token(&creds)
        .await
        .unwrap_or_else(|e| e.bail(2));

    seed_seen(&client, &token.access_token, &query, &mut seen_ids).await;

    // Poll loop
    loop {
        if std::time::Instant::now() >= deadline {
            output::print_json(&WaitTimeout {
                schema_version: output::SCHEMA_VERSION,
                matched: false,
                timeout,
            });
            std::process::exit(3);
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Refresh token each iteration in case it expired during a long wait
        let token = match refresh::get_access_token(&creds).await {
            Ok(t) => t,
            Err(_) => continue,
        };

        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages?q={}&maxResults=5",
            query
        );

        let resp = match client
            .get(&url)
            .bearer_auth(&token.access_token)
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => continue,
        };

        #[derive(serde::Deserialize)]
        struct ListResponse {
            messages: Option<Vec<MessageRef>>,
        }

        #[derive(serde::Deserialize)]
        struct MessageRef {
            id: String,
        }

        let list: ListResponse = match resp.json().await {
            Ok(l) => l,
            Err(_) => continue,
        };

        let refs = list.messages.unwrap_or_default();

        for msg_ref in refs {
            if seen_ids.contains(&msg_ref.id) {
                continue;
            }
            // New message - fetch and return it
            match fetch_message(&client, &token.access_token, &msg_ref.id).await {
                Ok(msg) => {
                    output::print_json(&msg);
                    return;
                }
                Err(_) => continue,
            }
        }
    }
}

async fn seed_seen(
    client: &reqwest::Client,
    access_token: &str,
    query: &str,
    seen: &mut std::collections::HashSet<String>,
) {
    let url = format!(
        "https://gmail.googleapis.com/gmail/v1/users/me/messages?q={}&maxResults=20",
        query
    );

    #[derive(serde::Deserialize)]
    struct ListResponse {
        messages: Option<Vec<MessageRef>>,
    }

    #[derive(serde::Deserialize)]
    struct MessageRef {
        id: String,
    }

    if let Ok(resp) = client.get(&url).bearer_auth(access_token).send().await {
        if let Ok(list) = resp.json::<ListResponse>().await {
            for m in list.messages.unwrap_or_default() {
                seen.insert(m.id);
            }
        }
    }
}
