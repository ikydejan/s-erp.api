use std::collections::HashMap;

use auth::password::PasswordPolicy;
use chrono::{DateTime, Utc};
use erp_core::PermissionLevel;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct UserRecord {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub password_changed_at: DateTime<Utc>,
    pub failed_login_count: i16,
    pub locked_until: Option<DateTime<Utc>>,
    pub mfa_enabled: bool,
    pub mfa_secret_encrypted: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserRecord {
    pub fn is_locked(&self) -> bool {
        self.locked_until.is_some_and(|until| until > Utc::now())
    }

    pub fn to_domain(&self) -> erp_core::User {
        erp_core::User {
            id: self.id,
            username: self.username.clone(),
            email: self.email.clone(),
            mfa_enabled: self.mfa_enabled,
            is_active: self.is_active,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub struct NewUser {
    pub username: String,
    pub email: String,
    pub password_hash: String,
}

#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

const DEFAULT_ROLE: &str = "staff";

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, new_user: NewUser) -> anyhow::Result<UserRecord> {
        let mut tx = self.pool.begin().await?;

        let user: UserRecord = sqlx::query_as(
            r#"
            INSERT INTO users (username, email, password_hash)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(&new_user.username)
        .bind(&new_user.email)
        .bind(&new_user.password_hash)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO password_history (user_id, password_hash)
            VALUES ($1, $2)
            "#,
        )
        .bind(user.id)
        .bind(&new_user.password_hash)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO user_roles (user_id, role_id)
            SELECT $1, id FROM roles WHERE name = $2
            "#,
        )
        .bind(user.id)
        .bind(DEFAULT_ROLE)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(user)
    }

    pub async fn find_by_username(&self, username: &str) -> anyhow::Result<Option<UserRecord>> {
        let user = sqlx::query_as("SELECT * FROM users WHERE username = $1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    pub async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<UserRecord>> {
        let user = sqlx::query_as("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    pub async fn record_login_success(&self, id: Uuid) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET failed_login_count = 0, locked_until = NULL WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Increments the failed-attempt counter and locks the account once it
    /// crosses the policy threshold. Returns the account's lock state after.
    pub async fn record_login_failure(&self, id: Uuid) -> anyhow::Result<bool> {
        let new_count: i16 = sqlx::query_scalar(
            "UPDATE users SET failed_login_count = failed_login_count + 1 WHERE id = $1 RETURNING failed_login_count",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        if PasswordPolicy::should_lock(new_count) {
            let locked_until = PasswordPolicy::lockout_expiry(Utc::now());
            sqlx::query("UPDATE users SET locked_until = $2 WHERE id = $1")
                .bind(id)
                .bind(locked_until)
                .execute(&self.pool)
                .await?;
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn recent_password_hashes(
        &self,
        user_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<String>> {
        let hashes =
            sqlx::query_scalar("SELECT password_hash FROM password_history WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2")
                .bind(user_id)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?;
        Ok(hashes)
    }

    pub async fn update_password(&self, user_id: Uuid, new_hash: &str) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "UPDATE users SET password_hash = $2, password_changed_at = now() WHERE id = $1",
        )
        .bind(user_id)
        .bind(new_hash)
        .execute(&mut *tx)
        .await?;

        sqlx::query("INSERT INTO password_history (user_id, password_hash) VALUES ($1, $2)")
            .bind(user_id)
            .bind(new_hash)
            .execute(&mut *tx)
            .await?;

        // Keep only the last `history_size` entries so the table doesn't grow forever.
        sqlx::query(
            r#"
            DELETE FROM password_history
            WHERE user_id = $1
              AND id NOT IN (
                  SELECT id FROM password_history
                  WHERE user_id = $1
                  ORDER BY created_at DESC
                  LIMIT $2
              )
            "#,
        )
        .bind(user_id)
        .bind(PasswordPolicy::history_size() as i64)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn set_mfa_secret(
        &self,
        user_id: Uuid,
        encrypted_secret: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET mfa_secret_encrypted = $2 WHERE id = $1")
            .bind(user_id)
            .bind(encrypted_secret)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_mfa_enabled(&self, user_id: Uuid, enabled: bool) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET mfa_enabled = $2 WHERE id = $1")
            .bind(user_id)
            .bind(enabled)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn store_backup_codes(
        &self,
        user_id: Uuid,
        code_hashes: &[String],
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM mfa_backup_codes WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        for hash in code_hashes {
            sqlx::query("INSERT INTO mfa_backup_codes (user_id, code_hash) VALUES ($1, $2)")
                .bind(user_id)
                .bind(hash)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Marks one unused backup code matching `code_hash` as used. Returns
    /// `true` if a matching, still-unused code was found and consumed.
    pub async fn consume_backup_code(
        &self,
        user_id: Uuid,
        code_hash: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE mfa_backup_codes
            SET used_at = now()
            WHERE id = (
                SELECT id FROM mfa_backup_codes
                WHERE user_id = $1 AND code_hash = $2 AND used_at IS NULL
                LIMIT 1
            )
            "#,
        )
        .bind(user_id)
        .bind(code_hash)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Effective permission per resource across every role the user holds
    /// (highest level wins — see `auth::rbac::merge_permissions`).
    pub async fn effective_permissions(
        &self,
        user_id: Uuid,
    ) -> anyhow::Result<HashMap<String, PermissionLevel>> {
        let rows: Vec<(String, i16)> = sqlx::query_as(
            r#"
            SELECT rp.resource, rp.level
            FROM user_roles ur
            JOIN role_permissions rp ON rp.role_id = ur.role_id
            WHERE ur.user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let levels = rows.into_iter().filter_map(|(resource, level)| {
            PermissionLevel::from_i16(level).map(|l| (resource, l))
        });
        Ok(auth::rbac::merge_permissions(levels))
    }

    pub async fn create_refresh_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (user_id, token_hash, expires_at, ip_address, user_agent)
            VALUES ($1, $2, $3, $4::text::inet, $5)
            "#,
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .bind(ip_address)
        .bind(user_agent)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns the owning `user_id` if the token hash is valid, unexpired, and
    /// not revoked.
    pub async fn find_valid_refresh_token(&self, token_hash: &str) -> anyhow::Result<Option<Uuid>> {
        let user_id = sqlx::query_scalar(
            r#"
            SELECT user_id FROM refresh_tokens
            WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now()
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user_id)
    }

    pub async fn revoke_refresh_token(&self, token_hash: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE refresh_tokens SET revoked_at = now() WHERE token_hash = $1")
            .bind(token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
