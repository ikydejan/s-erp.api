use totp_rs::{Algorithm, Secret, TOTP};

const ISSUER: &str = "S-ERP";

fn build(secret_base32: &str, account_email: &str) -> Result<TOTP, anyhow::Error> {
    let secret_bytes = Secret::Encoded(secret_base32.to_string())
        .to_bytes()
        .map_err(|e| anyhow::anyhow!("invalid MFA secret: {e:?}"))?;
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret_bytes,
        Some(ISSUER.to_string()),
        account_email.to_string(),
    )
    .map_err(|e| anyhow::anyhow!("failed to build TOTP: {e}"))
}

pub fn generate_secret_base32() -> String {
    Secret::generate_secret().to_encoded().to_string()
}

/// `otpauth://` URI the client renders as a QR code for the authenticator app.
pub fn provisioning_uri(secret_base32: &str, account_email: &str) -> Result<String, anyhow::Error> {
    Ok(build(secret_base32, account_email)?.get_url())
}

pub fn verify(secret_base32: &str, account_email: &str, code: &str) -> Result<bool, anyhow::Error> {
    let totp = build(secret_base32, account_email)?;
    Ok(totp.check_current(code).unwrap_or(false))
}
