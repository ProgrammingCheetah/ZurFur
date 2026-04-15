mod common;

use persistence::SqlxUserRepository;
use domain::user::UserRepository;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_user_from_atproto(pool: PgPool) {
    let repo = SqlxUserRepository::new(pool);

    let user = repo
        .create_from_atproto("did:plc:abc123", Some("alice.bsky.social"), Some("alice@example.com"))
        .await
        .expect("should create user");

    assert_eq!(user.did.as_deref(), Some("did:plc:abc123"));
    assert_eq!(user.handle.as_deref(), Some("alice.bsky.social"));
    assert_eq!(user.email.as_deref(), Some("alice@example.com"));
    assert_eq!(user.username, "alice");
    assert!(user.onboarding_completed_at.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_user_by_id(pool: PgPool) {
    let repo = SqlxUserRepository::new(pool);
    let user = repo
        .create_from_atproto("did:plc:find-by-id", Some("bob.bsky.social"), None)
        .await
        .unwrap();

    let found = repo.find_by_id(user.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, user.id);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_user_by_did(pool: PgPool) {
    let repo = SqlxUserRepository::new(pool);
    let user = repo
        .create_from_atproto("did:plc:find-by-did", Some("carol.bsky.social"), None)
        .await
        .unwrap();

    let found = repo.find_by_did("did:plc:find-by-did").await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, user.id);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_user_by_email(pool: PgPool) {
    let repo = SqlxUserRepository::new(pool);
    let user = repo
        .create_from_atproto("did:plc:find-by-email", Some("dan.bsky.social"), Some("dan@example.com"))
        .await
        .unwrap();

    let found = repo.find_by_email("dan@example.com").await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, user.id);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_nonexistent_user_returns_none(pool: PgPool) {
    let repo = SqlxUserRepository::new(pool);

    let found = repo.find_by_id(Uuid::new_v4()).await.unwrap();
    assert!(found.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn update_handle(pool: PgPool) {
    let repo = SqlxUserRepository::new(pool);
    let user = repo
        .create_from_atproto("did:plc:update-handle", Some("old.bsky.social"), None)
        .await
        .unwrap();

    repo.update_handle(user.id, "new.bsky.social").await.unwrap();

    let found = repo.find_by_id(user.id).await.unwrap().unwrap();
    assert_eq!(found.handle.as_deref(), Some("new.bsky.social"));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn mark_onboarding_completed(pool: PgPool) {
    let repo = SqlxUserRepository::new(pool);
    let user = repo
        .create_from_atproto("did:plc:onboarding", Some("eve.bsky.social"), None)
        .await
        .unwrap();

    assert!(user.onboarding_completed_at.is_none());

    repo.mark_onboarding_completed(user.id).await.unwrap();

    let found = repo.find_by_id(user.id).await.unwrap().unwrap();
    assert!(found.onboarding_completed_at.is_some());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_duplicate_did_fails(pool: PgPool) {
    let repo = SqlxUserRepository::new(pool);

    repo.create_from_atproto("did:plc:duplicate", Some("first.bsky.social"), None)
        .await
        .unwrap();

    let result = repo
        .create_from_atproto("did:plc:duplicate", Some("second.bsky.social"), None)
        .await;

    assert!(result.is_err(), "duplicate DID should fail");
}
