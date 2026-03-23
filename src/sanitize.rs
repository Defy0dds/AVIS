use crate::errors::AvisError;

/// Maximum attachment file size in bytes (20 MB).
const MAX_ATTACHMENT_SIZE: u64 = 20 * 1024 * 1024;

/// Reject a MIME header value that contains CR or LF (header injection).
pub fn validate_header_value(field: &str, value: &str) -> Result<(), AvisError> {
    if value.contains('\r') || value.contains('\n') {
        return Err(AvisError::new(
            "invalid_header_value",
            format!("{} contains illegal newline characters", field),
        ));
    }
    Ok(())
}

/// Check that a file does not exceed the safe attachment size limit.
pub fn check_attachment_size(path: &std::path::Path, display_name: &str) -> Result<(), AvisError> {
    let meta = std::fs::metadata(path)
        .map_err(|e| AvisError::new("attachment_read_error", format!("{}: {}", display_name, e)))?;
    if meta.len() > MAX_ATTACHMENT_SIZE {
        return Err(AvisError::new(
            "attachment_too_large",
            format!(
                "{} is {} bytes, exceeds 20 MB limit",
                display_name,
                meta.len()
            ),
        ));
    }
    Ok(())
}

/// RFC 5987 encode a filename for Content-Disposition.
/// If the filename is pure ASCII, returns `filename="name"`.
/// Otherwise returns `filename*=UTF-8''<percent-encoded>`.
pub fn encode_content_disposition_filename(name: &str) -> String {
    if name.is_ascii() {
        format!("filename=\"{}\"", name)
    } else {
        let encoded: String = name
            .bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_' {
                    (b as char).to_string()
                } else {
                    format!("%{:02X}", b)
                }
            })
            .collect();
        format!("filename*=UTF-8''{}", encoded)
    }
}

/// Encode a filename for the Content-Type name parameter.
/// Non-ASCII names are returned as a quoted ASCII-safe fallback.
pub fn encode_content_type_name(name: &str) -> String {
    if name.is_ascii() {
        format!("name=\"{}\"", name)
    } else {
        // Use "attachment" as a safe fallback for the name= param;
        // the real name is in Content-Disposition filename*
        "name=\"attachment\"".to_string()
    }
}

/// Sanitize a filename from an untrusted source (e.g. Gmail attachment metadata).
/// Extracts only the final path component and rejects empty or dot-only names.
pub fn sanitize_filename(raw: &str) -> Result<String, AvisError> {
    let leaf = std::path::Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if leaf.is_empty() || leaf.chars().all(|c| c == '.') {
        return Err(AvisError::new(
            "invalid_filename",
            format!("Attachment filename is invalid: {:?}", raw),
        ));
    }

    Ok(leaf.to_string())
}

/// Strip ASCII control characters from a string, keeping only `\n`, `\r`, and `\t`.
pub fn strip_control_chars(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            if c == '\n' || c == '\r' || c == '\t' {
                true
            } else {
                !c.is_ascii_control()
            }
        })
        .collect()
}
