use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

/// Encrypts the TOTP secret before it is persisted (AES-256-GCM).
///
/// `MFA_ENCRYPTION_KEY` must decode to exactly 32 raw bytes. This function only
/// covers the encrypt/decrypt half of "encryption at rest" — in production the
/// key itself belongs in a KMS/secrets manager with rotation, not a plain env
/// var; swap `load_key` for that once one is available.
pub fn encrypt(plaintext: &str, key_b64: &str) -> Result<String, anyhow::Error> {
    let cipher = load_cipher(key_b64)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("MFA secret encryption failed: {e}"))?;
    let mut combined = nonce.to_vec();
    combined.extend(ciphertext);
    Ok(STANDARD.encode(combined))
}

pub fn decrypt(ciphertext_b64: &str, key_b64: &str) -> Result<String, anyhow::Error> {
    let cipher = load_cipher(key_b64)?;
    let combined = STANDARD.decode(ciphertext_b64)?;
    if combined.len() < 12 {
        anyhow::bail!("ciphertext too short to contain a nonce");
    }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("MFA secret decryption failed: {e}"))?;
    Ok(String::from_utf8(plaintext)?)
}

fn load_cipher(key_b64: &str) -> Result<Aes256Gcm, anyhow::Error> {
    let key_bytes = STANDARD.decode(key_b64)?;
    if key_bytes.len() != 32 {
        anyhow::bail!("MFA_ENCRYPTION_KEY must decode to exactly 32 bytes");
    }
    Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| anyhow::anyhow!("invalid MFA encryption key: {e}"))
}
