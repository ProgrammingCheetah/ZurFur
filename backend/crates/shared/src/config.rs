use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    Missing(&'static str),
    #[error("Invalid environment variable: {0}")]
    Invalid(&'static str),
}

/// JWT configuration for Zurfur platform sessions.
pub struct JwtConfig {
    pub secret: Vec<u8>,
    /// Access token lifetime in seconds (default: 900 = 15 minutes).
    pub access_expiry_secs: u64,
    /// Refresh token lifetime in seconds (default: 2592000 = 30 days).
    pub refresh_expiry_secs: u64,
}

impl JwtConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let secret = std::env::var("JWT_SECRET")
            .map_err(|_| ConfigError::Missing("JWT_SECRET"))?;

        let access_expiry_secs = std::env::var("JWT_ACCESS_EXPIRY_SECS")
            .unwrap_or_else(|_| "900".into())
            .parse()
            .map_err(|_| ConfigError::Invalid("JWT_ACCESS_EXPIRY_SECS"))?;

        let refresh_expiry_secs = std::env::var("JWT_REFRESH_EXPIRY_SECS")
            .unwrap_or_else(|_| "2592000".into())
            .parse()
            .map_err(|_| ConfigError::Invalid("JWT_REFRESH_EXPIRY_SECS"))?;

        Ok(Self {
            secret: secret.into_bytes(),
            access_expiry_secs,
            refresh_expiry_secs,
        })
    }
}
