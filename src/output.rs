use serde::Serialize;

/// Print any serializable value to stdout as JSON.
/// This is the ONLY way commands should write to stdout.
pub fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{}", json),
        Err(e) => {
            eprintln!(
                r#"{{"schema_version":"1","error":"serialization_failed","message":"{}"}}"#,
                e
            );
            std::process::exit(2);
        }
    }
}

/// Shared schema_version field — included in every response.
pub const SCHEMA_VERSION: &str = "1";
