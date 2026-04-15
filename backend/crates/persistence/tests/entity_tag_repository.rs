mod common;

use common::*;
use domain::entity_tag::{EntityTagError, EntityTagRepository, TaggableEntityType};
use persistence::SqlxEntityTagRepository;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn attach_tag_to_entity(pool: PgPool) {
    let tag = create_test_tag(&pool, "general", "test-attach").await;
    let entity_id = Uuid::new_v4();
    let repo = SqlxEntityTagRepository::new(pool);

    let et = repo.attach(TaggableEntityType::Org, entity_id, tag.id).await.unwrap();

    assert_eq!(et.tag_id, tag.id);
    assert_eq!(et.entity_id, entity_id);
    assert!(matches!(et.entity_type, TaggableEntityType::Org));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn detach_tag(pool: PgPool) {
    let tag = create_test_tag(&pool, "general", "test-detach").await;
    let entity_id = Uuid::new_v4();
    let repo = SqlxEntityTagRepository::new(pool);

    repo.attach(TaggableEntityType::Org, entity_id, tag.id).await.unwrap();
    repo.detach(TaggableEntityType::Org, entity_id, tag.id).await.unwrap();

    let tags = repo.list_by_entity(TaggableEntityType::Org, entity_id).await.unwrap();
    assert!(tags.is_empty());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_entity(pool: PgPool) {
    let tag1 = create_test_tag(&pool, "general", "list-tag-1").await;
    let tag2 = create_test_tag(&pool, "metadata", "list-tag-2").await;
    let entity_id = Uuid::new_v4();
    let repo = SqlxEntityTagRepository::new(pool);

    repo.attach(TaggableEntityType::Org, entity_id, tag1.id).await.unwrap();
    repo.attach(TaggableEntityType::Org, entity_id, tag2.id).await.unwrap();

    let tags = repo.list_by_entity(TaggableEntityType::Org, entity_id).await.unwrap();
    assert_eq!(tags.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_tag(pool: PgPool) {
    let tag = create_test_tag(&pool, "general", "shared-tag").await;
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let repo = SqlxEntityTagRepository::new(pool);

    repo.attach(TaggableEntityType::Org, id1, tag.id).await.unwrap();
    repo.attach(TaggableEntityType::Commission, id2, tag.id).await.unwrap();

    let entities = repo.list_by_tag(tag.id).await.unwrap();
    assert_eq!(entities.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn attach_duplicate_fails(pool: PgPool) {
    let tag = create_test_tag(&pool, "general", "dup-attach").await;
    let entity_id = Uuid::new_v4();
    let repo = SqlxEntityTagRepository::new(pool);

    repo.attach(TaggableEntityType::Org, entity_id, tag.id).await.unwrap();
    let result = repo.attach(TaggableEntityType::Org, entity_id, tag.id).await;

    assert!(matches!(result, Err(EntityTagError::AlreadyAttached)));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn all_taggable_entity_types_accepted(pool: PgPool) {
    let repo = SqlxEntityTagRepository::new(pool.clone());

    for (et, name) in [
        (TaggableEntityType::Org, "org"),
        (TaggableEntityType::Commission, "commission"),
        (TaggableEntityType::FeedItem, "feed_item"),
        (TaggableEntityType::Character, "character"),
        (TaggableEntityType::FeedElement, "feed_element"),
    ] {
        let tag = create_test_tag(&pool, "general", &format!("et-{name}")).await;
        let result = repo.attach(et, Uuid::new_v4(), tag.id).await;
        assert!(result.is_ok(), "entity type '{name}' should be accepted");
    }
}
