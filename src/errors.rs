use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
pub struct AvisError {
    pub schema_version: &'static str,
    pub error: String,
    pub message: String,
}

impl AvisError {
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            schema_version: "1",
            error: error.into(),
            message: message.into(),
        }
    }

    /// Print this error to stderr as JSON and exit with the given code.
    pub fn bail(self, code: i32) -> ! {
        let json = serde_json::to_string(&self)
			.unwrap_or_else(|_| r#"{"schema_version":"1","error":"serialization_failed","message":"could not serialize error"}"#.to_string());
        eprintln!("{}", json);
        std::process::exit(code);
    }
}

impl fmt::Display for AvisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.error, self.message)
    }
}

impl std::error::Error for AvisError {}

// ── Convenience constructors ───────────────────────────────────────────────

impl AvisError {
    pub fn identity_not_found(name: &str) -> Self {
        Self::new(
            "identity_not_found",
            format!("No identity named '{}'. Run: avis ls", name),
        )
    }

    pub fn identity_exists(name: &str) -> Self {
        Self::new(
            "identity_exists",
            format!("Identity '{}' already exists.", name),
        )
    }

    pub fn credentials_corrupt(name: &str) -> Self {
        Self::new(
            "credentials_corrupt",
            format!(
                "credentials.enc for '{}' is corrupt or unreadable. Re-run: avis add id {} <email>",
                name, name
            ),
        )
    }

    pub fn oauth_failed(detail: impl Into<String>) -> Self {
        Self::new("oauth_failed", detail)
    }

    #[allow(dead_code)]
    pub fn token_revoked(name: &str) -> Self {
        Self::new(
            "token_revoked",
            format!(
                "Refresh token for '{}' has been revoked. Re-run: avis add id {} <email>",
                name, name
            ),
        )
    }

    #[allow(dead_code)]
    pub fn smtp_failure(detail: impl Into<String>) -> Self {
        Self::new("smtp_failure", detail)
    }

    pub fn imap_failure(detail: impl Into<String>) -> Self {
        Self::new("imap_failure", detail)
    }

    pub fn fs_error(detail: impl Into<String>) -> Self {
        Self::new("fs_error", detail)
    }
}
