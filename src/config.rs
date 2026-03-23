use crate::errors::AvisError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub home: PathBuf,
}

/// Resolve AVIS home in order:
/// 1. AVIS_HOME env var
/// 2. settings.json in platform config dir
/// 3. ~/.avis default
pub fn resolve_home() -> PathBuf {
    // 1. env var
    if let Ok(h) = std::env::var("AVIS_HOME") {
        return PathBuf::from(h);
    }

    // 2. persisted settings
    if let Some(cfg) = settings_file_path() {
        if cfg.exists() {
            if let Ok(raw) = std::fs::read_to_string(&cfg) {
                if let Ok(s) = serde_json::from_str::<Settings>(&raw) {
                    return s.home;
                }
            }
        }
    }

    // 3. default ~/.avis
    dirs_home().join(".avis")
}

/// Path to the platform config settings file.
/// Windows: %APPDATA%\avis\settings.json
/// Unix:    ~/.config/avis/settings.json
fn settings_file_path() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var("APPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("avis").join("settings.json"))
    } else {
        dirs_home()
            .parent()
            .map(|p| p.join(".config").join("avis").join("settings.json"))
            .or_else(|| {
                Some(
                    dirs_home()
                        .join(".config")
                        .join("avis")
                        .join("settings.json"),
                )
            })
    }
}

/// Persist the chosen home path to settings.json.
pub fn persist_home(home: &Path) -> Result<(), AvisError> {
    let settings = Settings {
        home: home.to_path_buf(),
    };
    let json =
        serde_json::to_string_pretty(&settings).map_err(|e| AvisError::fs_error(e.to_string()))?;

    if let Some(cfg) = settings_file_path() {
        if let Some(parent) = cfg.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AvisError::fs_error(e.to_string()))?;
        }
        std::fs::write(&cfg, json).map_err(|e| AvisError::fs_error(e.to_string()))?;
    }
    Ok(())
}

/// Returns the user's home directory.
fn dirs_home() -> PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

// ── Identity path helpers ──────────────────────────────────────────────────

pub fn identities_dir(home: &Path) -> PathBuf {
    home.join("identities")
}

pub fn identity_dir(home: &Path, name: &str) -> PathBuf {
    identities_dir(home).join(name)
}

pub fn identity_config_path(home: &Path, name: &str) -> PathBuf {
    identity_dir(home, name).join("config.json")
}

pub fn identity_credentials_path(home: &Path, name: &str) -> PathBuf {
    identity_dir(home, name).join("credentials.enc")
}

pub fn identity_master_key_path(home: &Path, name: &str) -> PathBuf {
    identity_dir(home, name).join("master.key")
}

pub fn logs_dir(home: &Path) -> PathBuf {
    home.join("logs")
}

/// Identity config stored in config.json
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdentityConfig {
    pub name: String,
    pub email: String,
    pub provider: String,
}

impl IdentityConfig {
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
            provider: "gmail".to_string(),
        }
    }
}

/// Load identity config. Returns error if identity doesn't exist.
pub fn load_identity(home: &Path, name: &str) -> Result<IdentityConfig, AvisError> {
    let path = identity_config_path(home, name);
    if !path.exists() {
        return Err(AvisError::identity_not_found(name));
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| AvisError::fs_error(e.to_string()))?;
    serde_json::from_str(&raw).map_err(|_| AvisError::credentials_corrupt(name))
}
