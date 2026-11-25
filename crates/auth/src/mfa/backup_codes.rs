use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

pub const BACKUP_CODE_COUNT: usize = 10;

/// Backup codes are high-entropy random strings (not user-chosen), so a fast
/// cryptographic hash is sufficient — unlike passwords, they don't need a
/// deliberately slow KDF to resist offline guessing.
pub fn generate_codes() -> Vec<String> {
    (0..BACKUP_CODE_COUNT).map(|_| generate_one()).collect()
}

fn generate_one() -> String {
    let mut bytes = [0u8; 6];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn hash_code(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn verify_code(code: &str, hash: &str) -> bool {
    hash_code(code) == hash
}
