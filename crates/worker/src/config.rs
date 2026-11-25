#[derive(Clone)]
pub struct WorkerConfig {
    pub database_url: String,
    pub opensearch_url: String,
    pub vapid_private_key: String,
    pub vapid_subject: String,
}

impl WorkerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: require_env("DATABASE_URL")?,
            opensearch_url: env_or("OPENSEARCH_URL", "http://localhost:9200"),
            vapid_private_key: std::env::var("VAPID_PRIVATE_KEY").unwrap_or_default(),
            vapid_subject: env_or("VAPID_SUBJECT", "mailto:admin@example.com"),
        })
    }
}

fn require_env(key: &str) -> anyhow::Result<String> {
    std::env::var(key).map_err(|_| anyhow::anyhow!("missing required env var {key}"))
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
