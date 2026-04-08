mod config;
pub mod oauth_state_store_memory;
mod pool;
mod repositories;
pub(crate) mod sqlx_utils;

pub use config::{Config, ConfigError};
pub use domain::atproto_session::{AtprotoSessionEntity, AtprotoSessionRepository};
pub use domain::content_rating::ContentRating;
pub use domain::oauth_state_store::OAuthStateStore;
pub use domain::organization::{Organization, OrganizationError, OrganizationRepository};
pub use domain::organization_member::{
    OrganizationMember, OrganizationMemberError, OrganizationMemberRepository, Permissions,
};
pub use domain::organization_profile::{
    CommissionStatus, OrganizationProfile, OrganizationProfileError, OrganizationProfileRepository,
};
pub use domain::refresh_token::{RefreshTokenEntity, RefreshTokenRepository};
pub use domain::user::{User, UserError, UserRepository};
pub use domain::user_preferences::{
    UserPreferences, UserPreferencesError, UserPreferencesRepository,
};
pub use pool::{create, Pool};
pub use repositories::{
    SqlxAtprotoSessionRepository, SqlxOrganizationMemberRepository,
    SqlxOrganizationProfileRepository, SqlxOrganizationRepository, SqlxRefreshTokenRepository,
    SqlxUserPreferencesRepository, SqlxUserRepository,
};

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
