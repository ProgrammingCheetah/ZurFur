use thiserror::Error;

/// Errors from configuration loading.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    Missing(&'static str),
    #[error("Invalid environment variable: {0}")]
    Invalid(&'static str),
}

/// JWT configuration for Zurfur platform sessions.
#[derive(Debug)]
pub struct JwtConfig {
    pub secret: Vec<u8>,
    /// Access token lifetime in seconds (default: 900 = 15 minutes).
    pub access_expiry_secs: u64,
    /// Refresh token lifetime in seconds (default: 2592000 = 30 days).
    pub refresh_expiry_secs: u64,
}

impl JwtConfig {
    /// Load JWT configuration from environment variables with defaults.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    // Env vars are process-global, so serialize tests that mutate them.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Guard that restores env vars on drop (including panics).
    struct EnvGuard {
        originals: Vec<(&'static str, Option<String>)>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, original) in &self.originals {
                match original {
                    Some(v) => unsafe { env::set_var(key, v) },
                    None => unsafe { env::remove_var(key) },
                }
            }
        }
    }

    fn with_env_vars<F: FnOnce() -> R, R>(vars: &[(&'static str, Option<&str>)], f: F) -> R {
        let _lock = ENV_LOCK.lock().unwrap();
        let mut guard = EnvGuard { originals: Vec::new() };
        for (key, val) in vars {
            guard.originals.push((key, env::var(key).ok()));
            match val {
                Some(v) => unsafe { env::set_var(key, v) },
                None => unsafe { env::remove_var(key) },
            }
        }
        f()
        // EnvGuard::drop restores originals, even on panic
    }

    #[test]
    fn jwt_config_loads_with_defaults() {
        with_env_vars(
            &[
                ("JWT_SECRET", Some("test-secret")),
                ("JWT_ACCESS_EXPIRY_SECS", None),
                ("JWT_REFRESH_EXPIRY_SECS", None),
            ],
            || {
                let config = JwtConfig::from_env().unwrap();
                assert_eq!(config.secret, b"test-secret");
                assert_eq!(config.access_expiry_secs, 900);
                assert_eq!(config.refresh_expiry_secs, 2592000);
            },
        );
    }

    #[test]
    fn jwt_config_loads_custom_expiry() {
        with_env_vars(
            &[
                ("JWT_SECRET", Some("s")),
                ("JWT_ACCESS_EXPIRY_SECS", Some("60")),
                ("JWT_REFRESH_EXPIRY_SECS", Some("3600")),
            ],
            || {
                let config = JwtConfig::from_env().unwrap();
                assert_eq!(config.access_expiry_secs, 60);
                assert_eq!(config.refresh_expiry_secs, 3600);
            },
        );
    }

    #[test]
    fn jwt_config_missing_secret_errors() {
        with_env_vars(&[("JWT_SECRET", None)], || {
            let err = JwtConfig::from_env().unwrap_err();
            assert!(matches!(err, ConfigError::Missing("JWT_SECRET")));
        });
    }

    #[test]
    fn jwt_config_invalid_expiry_errors() {
        with_env_vars(
            &[
                ("JWT_SECRET", Some("s")),
                ("JWT_ACCESS_EXPIRY_SECS", Some("not-a-number")),
            ],
            || {
                let err = JwtConfig::from_env().unwrap_err();
                assert!(matches!(err, ConfigError::Invalid("JWT_ACCESS_EXPIRY_SECS")));
            },
        );
    }
}
