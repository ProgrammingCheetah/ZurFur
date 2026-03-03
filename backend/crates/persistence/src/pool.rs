use sqlx::postgres::{PgConnectOptions, PgPool};
use std::str::FromStr;

use crate::config::Config;

/// PostgreSQL connection pool for the application.
pub type Pool = PgPool;

/// Creates and configures an SQLx connection pool, ready for use.
pub async fn create(config: &Config) -> Result<Pool, sqlx::Error> {
    let options = PgConnectOptions::from_str(&config.database_url)?;
    PgPool::connect_with(options).await
}
