#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub opensearch_url: String,
    pub host: String,
    pub port: u16,
    pub jwt_secret: String,
    pub jwt_access_ttl_minutes: i64,
    pub jwt_refresh_ttl_days: i64,
    pub mfa_encryption_key: String,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: require_env("DATABASE_URL")?,
            opensearch_url: env_or("OPENSEARCH_URL", "http://localhost:9200"),
            host: env_or("API_HOST", "0.0.0.0"),
            port: env_or("API_PORT", "8080").parse()?,
            jwt_secret: require_env("JWT_SECRET")?,
            jwt_access_ttl_minutes: env_or("JWT_ACCESS_TOKEN_TTL_MINUTES", "15").parse()?,
            jwt_refresh_ttl_days: env_or("JWT_REFRESH_TOKEN_TTL_DAYS", "7").parse()?,
            mfa_encryption_key: require_env("MFA_ENCRYPTION_KEY")?,
        })
    }
}

fn require_env(key: &str) -> anyhow::Result<String> {
    std::env::var(key).map_err(|_| anyhow::anyhow!("missing required env var {key}"))
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
