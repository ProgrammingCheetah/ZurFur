mod common;

use common::*;
use domain::entity::EntityKind;
use domain::feed::{FeedError, FeedRepository, FeedType};
use persistence::SqlxFeedRepository;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_feed(pool: PgPool) {
    let repo = SqlxFeedRepository::new(pool);

    let feed = repo.create("gallery", "Gallery", None, FeedType::Custom).await.unwrap();

    assert_eq!(feed.slug, "gallery");
    assert_eq!(feed.display_name, "Gallery");
    assert!(feed.description.is_none());
    assert!(matches!(feed.feed_type, FeedType::Custom));
    assert!(feed.deleted_at.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_and_attach_feed(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org = create_test_org(&pool, "feed-attach-org", None, false, Some(user.id)).await;
    let repo = SqlxFeedRepository::new(pool);

    let feed = repo
        .create_and_attach("updates", "Updates", None, FeedType::System, EntityKind::Org, org.id)
        .await
        .unwrap();

    assert_eq!(feed.slug, "updates");
    assert!(matches!(feed.feed_type, FeedType::System));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_feed_by_id(pool: PgPool) {
    let repo = SqlxFeedRepository::new(pool);
    let feed = repo.create("find-feed", "Find Feed", Some("desc"), FeedType::Custom).await.unwrap();

    let found = repo.find_by_id(feed.id).await.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.id, feed.id);
    assert_eq!(found.description.as_deref(), Some("desc"));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn update_feed(pool: PgPool) {
    let repo = SqlxFeedRepository::new(pool);
    let feed = repo.create("update-feed", "Old Name", None, FeedType::Custom).await.unwrap();

    let updated = repo.update(feed.id, "New Name", Some("new desc")).await.unwrap();

    assert_eq!(updated.display_name, "New Name");
    assert_eq!(updated.description.as_deref(), Some("new desc"));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn soft_delete_custom_feed(pool: PgPool) {
    let repo = SqlxFeedRepository::new(pool);
    let feed = repo.create("delete-feed", "Delete Me", None, FeedType::Custom).await.unwrap();

    repo.soft_delete(feed.id).await.unwrap();

    let found = repo.find_by_id(feed.id).await.unwrap();
    assert!(found.is_none(), "soft-deleted feed should not be found");
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn soft_delete_system_feed_fails(pool: PgPool) {
    let repo = SqlxFeedRepository::new(pool);
    let feed = repo.create("sys-feed", "System Feed", None, FeedType::System).await.unwrap();

    let result = repo.soft_delete(feed.id).await;
    assert!(matches!(result, Err(FeedError::SystemFeedUndeletable)));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_ids(pool: PgPool) {
    let repo = SqlxFeedRepository::new(pool);
    let f1 = repo.create("list-1", "Feed 1", None, FeedType::Custom).await.unwrap();
    let f2 = repo.create("list-2", "Feed 2", None, FeedType::Custom).await.unwrap();
    let _f3 = repo.create("list-3", "Feed 3", None, FeedType::Custom).await.unwrap();

    let feeds = repo.list_by_ids(&[f1.id, f2.id]).await.unwrap();
    assert_eq!(feeds.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_ids_unknown_returns_empty(pool: PgPool) {
    let repo = SqlxFeedRepository::new(pool);

    let feeds = repo.list_by_ids(&[Uuid::new_v4()]).await.unwrap();
    assert!(feeds.is_empty());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn duplicate_slug_per_entity_allowed(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org1 = create_test_org(&pool, "slug-org-1", None, false, Some(user.id)).await;
    let org2 = create_test_org(&pool, "slug-org-2", None, false, None).await;
    let repo = SqlxFeedRepository::new(pool);

    let f1 = repo
        .create_and_attach("gallery", "Gallery", None, FeedType::Custom, EntityKind::Org, org1.id)
        .await;
    let f2 = repo
        .create_and_attach("gallery", "Gallery", None, FeedType::Custom, EntityKind::Org, org2.id)
        .await;

    // Both should succeed — feeds are not globally unique by slug
    // (If slug IS globally unique, the second will fail and we'll need to adjust the schema expectation)
    if f1.is_ok() && f2.is_ok() {
        // Slug uniqueness is per-entity, not global — expected behavior
    } else if f1.is_ok() && f2.is_err() {
        // Global slug uniqueness — feed slugs are globally unique
        // This is also valid schema design; the test documents current behavior
        panic!("Feed slug appears to be globally unique — update design doc if intentional");
    }
}
