//! Shared test helpers for persistence integration tests.
//!
//! These insert directly via SQLx (not through repository implementations)
//! to avoid coupling test setup to the code under test.

use sqlx::PgPool;
use uuid::Uuid;

pub struct TestUser {
    pub id: Uuid,
    pub did: String,
    pub handle: String,
    pub email: String,
    pub username: String,
}

pub struct TestOrg {
    pub id: Uuid,
    pub slug: String,
    pub display_name: Option<String>,
    pub is_personal: bool,
}

pub struct TestFeed {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub feed_type: String,
}

pub struct TestFeedItem {
    pub id: Uuid,
    pub feed_id: Uuid,
}

/// Create a test user with unique random values.
pub async fn create_test_user(pool: &PgPool) -> TestUser {
    let id = Uuid::new_v4();
    let did = format!("did:plc:{}", Uuid::new_v4().simple());
    let handle = format!("test-{}.bsky.social", &id.to_string()[..8]);
    let email = format!("test-{}@example.com", &id.to_string()[..8]);
    let username = format!("test-{}", &id.to_string()[..8]);

    sqlx::query(
        "INSERT INTO users (id, did, handle, email, username) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(&did)
    .bind(&handle)
    .bind(&email)
    .bind(&username)
    .execute(pool)
    .await
    .expect("failed to create test user");

    TestUser { id, did, handle, email, username }
}

/// Create a test organization. If `owner_id` is provided, also adds an owner membership.
pub async fn create_test_org(
    pool: &PgPool,
    slug: &str,
    display_name: Option<&str>,
    is_personal: bool,
    owner_id: Option<Uuid>,
) -> TestOrg {
    let id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO organization (id, slug, display_name, is_personal) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(slug)
    .bind(display_name)
    .bind(is_personal)
    .execute(pool)
    .await
    .expect("failed to create test org");

    if let Some(uid) = owner_id {
        // permissions = -1 i64 → u64::MAX (all bits set) for owner
        sqlx::query(
            "INSERT INTO organization_member (org_id, user_id, role, permissions) \
             VALUES ($1, $2, 'owner', -1)",
        )
        .bind(id)
        .bind(uid)
        .execute(pool)
        .await
        .expect("failed to add owner member");
    }

    TestOrg {
        id,
        slug: slug.to_string(),
        display_name: display_name.map(String::from),
        is_personal,
    }
}

/// Create a test feed and optionally attach it to an entity.
pub async fn create_test_feed(
    pool: &PgPool,
    slug: &str,
    display_name: &str,
    feed_type: &str,
    attach_to: Option<(&str, Uuid)>,
) -> TestFeed {
    let id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO feed (id, slug, display_name, feed_type) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(slug)
    .bind(display_name)
    .bind(feed_type)
    .execute(pool)
    .await
    .expect("failed to create test feed");

    if let Some((entity_type, entity_id)) = attach_to {
        sqlx::query(
            "INSERT INTO entity_feed (feed_id, entity_type, entity_id) VALUES ($1, $2, $3)",
        )
        .bind(id)
        .bind(entity_type)
        .bind(entity_id)
        .execute(pool)
        .await
        .expect("failed to attach test feed");
    }

    TestFeed {
        id,
        slug: slug.to_string(),
        display_name: display_name.to_string(),
        feed_type: feed_type.to_string(),
    }
}

/// Create a test feed item.
pub async fn create_test_feed_item(
    pool: &PgPool,
    feed_id: Uuid,
    author_type: &str,
    author_id: Uuid,
) -> TestFeedItem {
    let id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO feed_item (id, feed_id, author_type, author_id) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(feed_id)
    .bind(author_type)
    .bind(author_id)
    .execute(pool)
    .await
    .expect("failed to create test feed item");

    TestFeedItem { id, feed_id }
}
