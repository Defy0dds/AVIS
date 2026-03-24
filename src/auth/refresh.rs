use crate::errors::AvisError;
use serde::{Deserialize, Serialize};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Stored in credentials.enc after successful OAuth2 flow
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OAuthCredentials {
    pub refresh_token: String,
    pub client_id: String,
    pub client_secret: String,
}

/// Short-lived access token returned by Google
#[derive(Debug, Deserialize)]
pub struct AccessToken {
    pub access_token: String,
}

/// Exchange authorization code for refresh token (called once during identity add)
pub async fn exchange_code(
    code: &str,
    code_verifier: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
) -> Result<OAuthCredentials, AvisError> {
    let client = reqwest::Client::new();

    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("code_verifier", code_verifier),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", redirect_uri),
    ];
    let body =
        serde_urlencoded::to_string(params).map_err(|e| AvisError::oauth_failed(e.to_string()))?;

    let resp = client
        .post(TOKEN_URL)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .map_err(|e| AvisError::oauth_failed(e.to_string()))?;

    #[derive(Deserialize)]
    struct TokenResponse {
        refresh_token: Option<String>,
        error: Option<String>,
        error_description: Option<String>,
    }

    let body: TokenResponse = resp
        .json()
        .await
        .map_err(|e| AvisError::oauth_failed(e.to_string()))?;

    if let Some(err) = body.error {
        let desc = body.error_description.unwrap_or_default();
        return Err(AvisError::oauth_failed(format!("{}: {}", err, desc)));
    }

    let refresh_token = body
        .refresh_token
        .ok_or_else(|| AvisError::oauth_failed("Google did not return a refresh token"))?;

    Ok(OAuthCredentials {
        refresh_token,
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
    })
}

/// Get a fresh access token using the stored refresh token
pub async fn get_access_token(creds: &OAuthCredentials) -> Result<AccessToken, AvisError> {
    let client = reqwest::Client::new();

    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", creds.refresh_token.as_str()),
        ("client_id", creds.client_id.as_str()),
        ("client_secret", creds.client_secret.as_str()),
    ];
    let body =
        serde_urlencoded::to_string(params).map_err(|e| AvisError::imap_failure(e.to_string()))?;

    let resp = client
        .post(TOKEN_URL)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .map_err(|e| AvisError::imap_failure(e.to_string()))?;

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: Option<String>,
        error: Option<String>,
    }

    let body: TokenResponse = resp
        .json()
        .await
        .map_err(|e| AvisError::imap_failure(e.to_string()))?;

    if let Some(err) = body.error {
        if err.contains("invalid_grant") {
            return Err(AvisError::new(
                "token_revoked",
                "Refresh token revoked. Re-run: avis add <name>",
            ));
        }
        return Err(AvisError::imap_failure(format!(
            "Token refresh failed: {}",
            err
        )));
    }

    let access_token = body
        .access_token
        .ok_or_else(|| AvisError::imap_failure("No access token in refresh response"))?;

    Ok(AccessToken { access_token })
}
