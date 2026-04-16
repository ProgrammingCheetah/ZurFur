mod common;

use common::*;
use domain::entity_feed::{EntityFeedError, EntityFeedRepository, EntityType};
use persistence::SqlxEntityFeedRepository;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn attach_feed_to_entity(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org = create_test_org(&pool, "ef-attach-org", None, false, Some(user.id)).await;
    let feed = create_test_feed(&pool, "ef-feed", "Test Feed", "custom", None).await;
    let repo = SqlxEntityFeedRepository::new(pool);

    let ef = repo.attach(feed.id, EntityType::Org, org.id).await.unwrap();

    assert_eq!(ef.feed_id, feed.id);
    assert_eq!(ef.entity_id, org.id);
    assert!(matches!(ef.entity_type, EntityType::Org));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_by_feed_id(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org = create_test_org(&pool, "ef-find-org", None, false, Some(user.id)).await;
    let feed = create_test_feed(&pool, "ef-find-feed", "Find Feed", "system", Some(("org", org.id))).await;
    let repo = SqlxEntityFeedRepository::new(pool);

    let found = repo.find_by_feed_id(feed.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().entity_id, org.id);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_entity(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org = create_test_org(&pool, "ef-list-org", None, false, Some(user.id)).await;
    let _f1 = create_test_feed(&pool, "ef-list-1", "Feed 1", "system", Some(("org", org.id))).await;
    let _f2 = create_test_feed(&pool, "ef-list-2", "Feed 2", "custom", Some(("org", org.id))).await;
    let repo = SqlxEntityFeedRepository::new(pool);

    let feeds = repo.list_by_entity(EntityType::Org, org.id).await.unwrap();
    assert_eq!(feeds.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn detach_feed(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org = create_test_org(&pool, "ef-detach-org", None, false, Some(user.id)).await;
    let feed = create_test_feed(&pool, "ef-detach", "Detach Feed", "custom", Some(("org", org.id))).await;
    let repo = SqlxEntityFeedRepository::new(pool);

    repo.detach(feed.id).await.unwrap();

    let found = repo.find_by_feed_id(feed.id).await.unwrap();
    assert!(found.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn attach_already_attached_fails(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org = create_test_org(&pool, "ef-dup-org", None, false, Some(user.id)).await;
    let feed = create_test_feed(&pool, "ef-dup", "Dup Feed", "custom", Some(("org", org.id))).await;
    let repo = SqlxEntityFeedRepository::new(pool);

    // Already attached via create_test_feed; try again
    let result = repo.attach(feed.id, EntityType::Org, org.id).await;
    assert!(matches!(result, Err(EntityFeedError::AlreadyAttached)));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn all_entity_types_accepted(pool: PgPool) {
    let repo = SqlxEntityFeedRepository::new(pool.clone());

    for (entity_type, type_str) in [
        (EntityType::Org, "org"),
        (EntityType::User, "user"),
        (EntityType::Character, "character"),
        (EntityType::Commission, "commission"),
    ] {
        let feed = create_test_feed(
            &pool,
            &format!("et-{type_str}"),
            &format!("Feed {type_str}"),
            "custom",
            None,
        )
        .await;
        let entity_id = Uuid::new_v4();

        let result = repo.attach(feed.id, entity_type, entity_id).await;
        assert!(result.is_ok(), "entity type '{type_str}' should be accepted");
    }
}
