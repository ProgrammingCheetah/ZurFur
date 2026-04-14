/// Database configuration for SQLx.
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
}

impl Config {
    /// Creates config from `DATABASE_URL` environment variable.
    pub fn from_env() -> Result<Self, ConfigError> {
        std::env::var("DATABASE_URL").map(|database_url| Self { database_url }).map_err(|_| ConfigError::MissingDatabaseUrl)
    }

    /// Creates config with an explicit database URL.
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
        }
    }
}

/// Errors from persistence configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("DATABASE_URL environment variable is not set")]
    MissingDatabaseUrl,
}
