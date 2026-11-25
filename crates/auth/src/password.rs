use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use chrono::{DateTime, Duration, Utc};

/// ISO/IEC 27001:2022 A.5.17-aligned password policy. We intentionally do not
/// force periodic rotation (deprecated by modern guidance, e.g. NIST 800-63B);
/// `password_changed_at` is tracked so a rotation policy can be layered on later
/// without a schema change.
pub struct PasswordPolicy;

const MIN_LENGTH: usize = 12;
const MAX_FAILED_ATTEMPTS: i16 = 5;
const LOCKOUT_MINUTES: i64 = 15;
const HISTORY_SIZE: usize = 5;

impl PasswordPolicy {
    /// Syntactic validation only (length + character classes). Reuse-against-history
    /// is checked separately by the caller, which has access to `password_history`.
    pub fn validate(password: &str) -> Result<(), String> {
        if password.chars().count() < MIN_LENGTH {
            return Err(format!("password must be at least {MIN_LENGTH} characters"));
        }
        let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        let has_symbol = password.chars().any(|c| !c.is_alphanumeric());
        if !(has_upper && has_lower && has_digit && has_symbol) {
            return Err("password must include an uppercase letter, a lowercase letter, a digit, and a symbol".into());
        }
        Ok(())
    }

    pub fn history_size() -> usize {
        HISTORY_SIZE
    }

    pub fn max_failed_attempts() -> i16 {
        MAX_FAILED_ATTEMPTS
    }

    /// Whether the account should be locked given the failed-attempt count that
    /// will result from the current failure (i.e. called with `failed_count + 1`).
    pub fn should_lock(failed_count_after_this_attempt: i16) -> bool {
        failed_count_after_this_attempt >= MAX_FAILED_ATTEMPTS
    }

    pub fn lockout_expiry(now: DateTime<Utc>) -> DateTime<Utc> {
        now + Duration::minutes(LOCKOUT_MINUTES)
    }
}

pub fn hash_password(password: &str) -> Result<String, anyhow::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}
