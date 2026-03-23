use crate::{config, output};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct InitResult {
    schema_version: &'static str,
    created: bool,
    home: String,
}

pub async fn run(home: &Path) {
    let created = !home.exists();

    // Create directory layout
    let dirs = [
        home.to_path_buf(),
        config::identities_dir(home),
        config::logs_dir(home),
    ];

    for dir in &dirs {
        if let Err(e) = std::fs::create_dir_all(dir) {
            crate::errors::AvisError::fs_error(format!(
                "Failed to create {}: {}",
                dir.display(),
                e
            ))
            .bail(2);
        }
    }

    // Persist home path to settings.json
    if let Err(e) = config::persist_home(home) {
        e.bail(2);
    }

    output::print_json(&InitResult {
        schema_version: output::SCHEMA_VERSION,
        created,
        home: home.display().to_string(),
    });
}
