mod common;

use common::*;
use domain::feed_item::{AuthorType, FeedItemRepository};
use persistence::SqlxFeedItemRepository;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_feed_item(pool: PgPool) {
    let feed = create_test_feed(&pool, "fi-feed", "Items Feed", "custom", None).await;
    let author_id = Uuid::new_v4();
    let repo = SqlxFeedItemRepository::new(pool);

    let item = repo.create(feed.id, AuthorType::User, author_id).await.unwrap();

    assert_eq!(item.feed_id, feed.id);
    assert_eq!(item.author_id, author_id);
    assert!(matches!(item.author_type, AuthorType::User));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_by_id(pool: PgPool) {
    let feed = create_test_feed(&pool, "fi-find", "Find Feed", "custom", None).await;
    let repo = SqlxFeedItemRepository::new(pool);
    let item = repo.create(feed.id, AuthorType::System, Uuid::new_v4()).await.unwrap();

    let found = repo.find_by_id(item.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, item.id);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_feed(pool: PgPool) {
    let feed = create_test_feed(&pool, "fi-list", "List Feed", "custom", None).await;
    let repo = SqlxFeedItemRepository::new(pool);

    for _ in 0..3 {
        repo.create(feed.id, AuthorType::User, Uuid::new_v4()).await.unwrap();
    }

    let items = repo.list_by_feed(feed.id, 10, 0).await.unwrap();
    assert_eq!(items.len(), 3);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_feed_empty(pool: PgPool) {
    let feed = create_test_feed(&pool, "fi-empty", "Empty Feed", "custom", None).await;
    let repo = SqlxFeedItemRepository::new(pool);

    let items = repo.list_by_feed(feed.id, 10, 0).await.unwrap();
    assert!(items.is_empty());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_feed_pagination(pool: PgPool) {
    let feed = create_test_feed(&pool, "fi-page", "Paginated Feed", "custom", None).await;
    let repo = SqlxFeedItemRepository::new(pool);

    for _ in 0..5 {
        repo.create(feed.id, AuthorType::User, Uuid::new_v4()).await.unwrap();
    }

    let page = repo.list_by_feed(feed.id, 2, 2).await.unwrap();
    assert_eq!(page.len(), 2, "limit=2 offset=2 should return 2 items");
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_with_elements(pool: PgPool) {
    use domain::feed_element::FeedElementType;
    use domain::feed_item::NewFeedElementInput;

    let feed = create_test_feed(&pool, "fi-elems", "Elements Feed", "custom", None).await;
    let author_id = Uuid::new_v4();
    let repo = SqlxFeedItemRepository::new(pool);

    let elements = vec![
        NewFeedElementInput {
            element_type: FeedElementType::Text,
            content_json: r#"{"text":"hello world"}"#.to_string(),
            position: 0,
        },
        NewFeedElementInput {
            element_type: FeedElementType::Image,
            content_json: r#"{"url":"img.png"}"#.to_string(),
            position: 1,
        },
    ];

    let (item, created_elements) = repo
        .create_with_elements(feed.id, AuthorType::User, author_id, &elements)
        .await
        .unwrap();

    assert_eq!(item.feed_id, feed.id);
    assert_eq!(item.author_id, author_id);
    assert_eq!(created_elements.len(), 2);
    assert_eq!(created_elements[0].position, 0);
    assert!(matches!(created_elements[0].element_type, FeedElementType::Text));
    assert_eq!(created_elements[1].position, 1);
    assert!(matches!(created_elements[1].element_type, FeedElementType::Image));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn delete_item(pool: PgPool) {
    let feed = create_test_feed(&pool, "fi-del", "Delete Feed", "custom", None).await;
    let repo = SqlxFeedItemRepository::new(pool);
    let item = repo.create(feed.id, AuthorType::User, Uuid::new_v4()).await.unwrap();

    repo.delete(item.id).await.unwrap();

    let found = repo.find_by_id(item.id).await.unwrap();
    assert!(found.is_none());
}
