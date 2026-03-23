use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};

pub struct PkceChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
}

impl PkceChallenge {
    pub fn new() -> Self {
        // Generate 32 random bytes -> base64url encode -> code_verifier
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let code_verifier = URL_SAFE_NO_PAD.encode(bytes);

        // SHA256(code_verifier) -> base64url encode -> code_challenge
        let digest = Sha256::digest(code_verifier.as_bytes());
        let code_challenge = URL_SAFE_NO_PAD.encode(digest);

        Self {
            code_verifier,
            code_challenge,
        }
    }
}
