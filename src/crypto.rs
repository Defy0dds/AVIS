use crate::errors::AvisError;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;

/// Generate a new 32-byte master key and write it to disk.
pub fn generate_master_key(path: &std::path::Path) -> Result<[u8; 32], AvisError> {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    std::fs::write(path, key)
        .map_err(|e| AvisError::fs_error(format!("Failed to write master.key: {}", e)))?;
    Ok(key)
}

/// Load existing master key from disk.
pub fn load_master_key(path: &std::path::Path) -> Result<[u8; 32], AvisError> {
    let bytes = std::fs::read(path)
        .map_err(|e| AvisError::fs_error(format!("Failed to read master.key: {}", e)))?;
    bytes
        .try_into()
        .map_err(|_| AvisError::credentials_corrupt("master.key is not 32 bytes"))
}

/// Encrypt plaintext using ChaCha20-Poly1305.
/// Output format: [12-byte nonce][ciphertext]
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, AvisError> {
    let cipher = ChaCha20Poly1305::new(key.into());

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| AvisError::fs_error("Encryption failed"))?;

    // Prepend nonce to ciphertext
    let mut output = nonce_bytes.to_vec();
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypt data encrypted by `encrypt()`.
/// Expects: [12-byte nonce][ciphertext]
pub fn decrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, AvisError> {
    if data.len() < 12 {
        return Err(AvisError::credentials_corrupt(
            "credentials.enc is too short",
        ));
    }

    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = ChaCha20Poly1305::new(key.into());

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AvisError::credentials_corrupt("credentials.enc decryption failed"))
}
