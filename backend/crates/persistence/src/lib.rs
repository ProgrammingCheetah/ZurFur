mod config;
pub mod oauth_state_store_memory;
mod pool;
mod repositories;
pub(crate) mod sqlx_utils;

pub use config::{Config, ConfigError};
pub use domain::atproto_session::{AtprotoSessionEntity, AtprotoSessionRepository};
pub use domain::content_rating::ContentRating;
pub use domain::default_role::{DefaultRole, DefaultRoleError, DefaultRoleRepository};
pub use domain::entity::EntityKind;
pub use domain::entity_feed::{EntityFeed, EntityFeedError, EntityFeedRepository};
pub use domain::entity_tag::{EntityTag, EntityTagError, EntityTagRepository};
pub use domain::feed::{Feed, FeedError, FeedRepository, FeedType};
pub use domain::feed_element::{FeedElement, FeedElementError, FeedElementRepository, FeedElementType};
pub use domain::feed_item::{AuthorType, FeedItem, FeedItemError, FeedItemRepository};
pub use domain::feed_subscription::{
    FeedSubscription, FeedSubscriptionError, FeedSubscriptionRepository, SubscriptionPermission,
};
pub use domain::oauth_state_store::OAuthStateStore;
pub use domain::onboarding_role::OnboardingRole;
pub use domain::organization::{Organization, OrganizationError, OrganizationRepository};
pub use domain::organization_member::{
    OrganizationMember, OrganizationMemberError, OrganizationMemberRepository, Permissions, Role,
};
pub use domain::refresh_token::{RefreshTokenEntity, RefreshTokenRepository};
pub use domain::tag::{Tag, TagCategory, TagError, TagRepository};
pub use domain::user::{User, UserError, UserRepository};
pub use domain::user_preferences::{
    UserPreferences, UserPreferencesError, UserPreferencesRepository,
};
pub use pool::{create, Pool};
pub use repositories::{
    SqlxAtprotoSessionRepository, SqlxDefaultRoleRepository, SqlxEntityFeedRepository,
    SqlxFeedElementRepository, SqlxFeedItemRepository, SqlxFeedRepository,
    SqlxEntityTagRepository, SqlxFeedSubscriptionRepository, SqlxOrganizationMemberRepository,
    SqlxOrganizationRepository, SqlxRefreshTokenRepository, SqlxTagRepository,
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

/// Compiled migrations, usable by both `migrate()` and `#[sqlx::test(migrator)]`.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Runs all pending migrations.
pub async fn migrate(pool: &Pool) -> Result<(), sqlx::Error> {
    MIGRATOR.run(pool).await.map_err(Into::into)
}
