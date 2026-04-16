mod common;

use common::*;
use domain::entity::EntityKind;
use domain::entity_tag::{EntityTagError, EntityTagRepository};
use persistence::SqlxEntityTagRepository;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn attach_tag_to_entity(pool: PgPool) {
    let tag = create_test_tag(&pool, "general", "test-attach").await;
    let entity_id = Uuid::new_v4();
    let repo = SqlxEntityTagRepository::new(pool);

    let et = repo.attach(EntityKind::Org, entity_id, tag.id).await.unwrap();

    assert_eq!(et.tag_id, tag.id);
    assert_eq!(et.entity_id, entity_id);
    assert!(matches!(et.entity_type, EntityKind::Org));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn detach_tag(pool: PgPool) {
    let tag = create_test_tag(&pool, "general", "test-detach").await;
    let entity_id = Uuid::new_v4();
    let repo = SqlxEntityTagRepository::new(pool);

    repo.attach(EntityKind::Org, entity_id, tag.id).await.unwrap();
    repo.detach(EntityKind::Org, entity_id, tag.id).await.unwrap();

    let tags = repo.list_by_entity(EntityKind::Org, entity_id).await.unwrap();
    assert!(tags.is_empty());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_entity(pool: PgPool) {
    let tag1 = create_test_tag(&pool, "general", "list-tag-1").await;
    let tag2 = create_test_tag(&pool, "metadata", "list-tag-2").await;
    let entity_id = Uuid::new_v4();
    let repo = SqlxEntityTagRepository::new(pool);

    repo.attach(EntityKind::Org, entity_id, tag1.id).await.unwrap();
    repo.attach(EntityKind::Org, entity_id, tag2.id).await.unwrap();

    let tags = repo.list_by_entity(EntityKind::Org, entity_id).await.unwrap();
    assert_eq!(tags.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_tag(pool: PgPool) {
    let tag = create_test_tag(&pool, "general", "shared-tag").await;
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let repo = SqlxEntityTagRepository::new(pool);

    repo.attach(EntityKind::Org, id1, tag.id).await.unwrap();
    repo.attach(EntityKind::Commission, id2, tag.id).await.unwrap();

    let entities = repo.list_by_tag(tag.id).await.unwrap();
    assert_eq!(entities.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn attach_duplicate_fails(pool: PgPool) {
    let tag = create_test_tag(&pool, "general", "dup-attach").await;
    let entity_id = Uuid::new_v4();
    let repo = SqlxEntityTagRepository::new(pool);

    repo.attach(EntityKind::Org, entity_id, tag.id).await.unwrap();
    let result = repo.attach(EntityKind::Org, entity_id, tag.id).await;

    assert!(matches!(result, Err(EntityTagError::AlreadyAttached)));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn all_taggable_entity_types_accepted(pool: PgPool) {
    let repo = SqlxEntityTagRepository::new(pool.clone());

    for (et, name) in [
        (EntityKind::Org, "org"),
        (EntityKind::Commission, "commission"),
        (EntityKind::FeedItem, "feed_item"),
        (EntityKind::Character, "character"),
        (EntityKind::FeedElement, "feed_element"),
        (EntityKind::User, "user"),
        (EntityKind::Tag, "tag"),
        (EntityKind::Feed, "feed"),
    ] {
        let tag = create_test_tag(&pool, "general", &format!("et-{name}")).await;
        let result = repo.attach(et, Uuid::new_v4(), tag.id).await;
        assert!(result.is_ok(), "entity type '{name}' should be accepted");
    }
}
