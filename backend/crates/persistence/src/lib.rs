mod config;
mod pool;
mod repositories;

pub use config::{Config, ConfigError};
pub use pool::{create, Pool};
pub use domain::user::{User, UserError, UserRepository};
pub use repositories::SqlxUserRepository;

/// Configures SQLx and returns a ready-to-use database pool.
///
/// # Example
/// ```ignore
/// use persistence::{Config, connect, SqlxUserRepository};
///
/// let config = Config::from_env()?;
/// let pool = connect(&config).await?;
/// let user_repo = SqlxUserRepository::from_pool(pool);
/// ```
pub async fn connect(config: &Config) -> Result<Pool, sqlx::Error> {
    let pool = pool::create(config).await?;
    migrate(&pool).await?;
    Ok(pool)
}

/// Runs all pending migrations.
pub async fn migrate(pool: &Pool) -> Result<(), sqlx::Error> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(Into::into)
}
