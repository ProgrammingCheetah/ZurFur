mod common;

use common::*;
use domain::feed_element::{FeedElementRepository, FeedElementType};
use persistence::SqlxFeedElementRepository;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_element(pool: PgPool) {
    let feed = create_test_feed(&pool, "fe-feed", "Element Feed", "custom", None).await;
    let item = create_test_feed_item(&pool, feed.id, "user", Uuid::new_v4()).await;
    let repo = SqlxFeedElementRepository::new(pool);

    let elem = repo
        .create(item.id, FeedElementType::Text, r#"{"text":"hello"}"#, 0)
        .await
        .unwrap();

    assert_eq!(elem.feed_item_id, item.id);
    assert!(matches!(elem.element_type, FeedElementType::Text));
    assert_eq!(elem.position, 0);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_by_id(pool: PgPool) {
    let feed = create_test_feed(&pool, "fe-find", "Find Feed", "custom", None).await;
    let item = create_test_feed_item(&pool, feed.id, "user", Uuid::new_v4()).await;
    let repo = SqlxFeedElementRepository::new(pool);
    let elem = repo.create(item.id, FeedElementType::Image, r#"{"url":"img.png"}"#, 0).await.unwrap();

    let found = repo.find_by_id(elem.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, elem.id);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_feed_item_ordered_by_position(pool: PgPool) {
    let feed = create_test_feed(&pool, "fe-list", "List Feed", "custom", None).await;
    let item = create_test_feed_item(&pool, feed.id, "user", Uuid::new_v4()).await;
    let repo = SqlxFeedElementRepository::new(pool);

    repo.create(item.id, FeedElementType::Text, r#"{"text":"second"}"#, 1).await.unwrap();
    repo.create(item.id, FeedElementType::Text, r#"{"text":"first"}"#, 0).await.unwrap();

    let elements = repo.list_by_feed_item(item.id).await.unwrap();
    assert_eq!(elements.len(), 2);
    assert_eq!(elements[0].position, 0);
    assert_eq!(elements[1].position, 1);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn update_content(pool: PgPool) {
    let feed = create_test_feed(&pool, "fe-update", "Update Feed", "custom", None).await;
    let item = create_test_feed_item(&pool, feed.id, "user", Uuid::new_v4()).await;
    let repo = SqlxFeedElementRepository::new(pool);
    let elem = repo.create(item.id, FeedElementType::Text, r#"{"text":"old"}"#, 0).await.unwrap();

    let updated = repo.update_content(elem.id, r#"{"text":"new"}"#).await.unwrap();
    assert_eq!(updated.content_json, r#"{"text":"new"}"#);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn delete_element(pool: PgPool) {
    let feed = create_test_feed(&pool, "fe-del", "Delete Feed", "custom", None).await;
    let item = create_test_feed_item(&pool, feed.id, "user", Uuid::new_v4()).await;
    let repo = SqlxFeedElementRepository::new(pool);
    let elem = repo.create(item.id, FeedElementType::Text, r#"{"text":"bye"}"#, 0).await.unwrap();

    repo.delete(elem.id).await.unwrap();

    let found = repo.find_by_id(elem.id).await.unwrap();
    assert!(found.is_none());
}
