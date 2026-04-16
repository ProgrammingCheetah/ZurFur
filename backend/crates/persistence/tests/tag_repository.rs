use domain::entity::EntityKind;
use domain::tag::{TagCategory, TagError, TagRepository};
use persistence::SqlxTagRepository;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_tag(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);

    let tag = repo.create(TagCategory::General, "furry", true).await.unwrap();

    assert_eq!(tag.name, "furry");
    assert!(matches!(tag.category, TagCategory::General));
    assert_eq!(tag.usage_count, 0);
    assert!(tag.is_approved);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_and_attach_tag(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);
    let entity_id = Uuid::new_v4();

    let tag = repo
        .create_and_attach(TagCategory::Organization, "studio-x", true, EntityKind::Org, entity_id)
        .await
        .unwrap();

    assert_eq!(tag.name, "studio-x");
    assert_eq!(tag.usage_count, 1, "usage_count should be incremented to 1");
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_tag_by_id(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);
    let tag = repo.create(TagCategory::General, "find-me", true).await.unwrap();

    let found = repo.find_by_id(tag.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, tag.id);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_by_name_and_category(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);
    repo.create(TagCategory::Metadata, "species:wolf", true).await.unwrap();

    let found = repo.find_by_name_and_category("species:wolf", TagCategory::Metadata).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "species:wolf");

    // Wrong category returns None
    let not_found = repo.find_by_name_and_category("species:wolf", TagCategory::General).await.unwrap();
    assert!(not_found.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_category(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);
    repo.create(TagCategory::General, "cat-a", true).await.unwrap();
    repo.create(TagCategory::General, "cat-b", true).await.unwrap();
    repo.create(TagCategory::Metadata, "other", true).await.unwrap();

    let tags = repo.list_by_category(TagCategory::General, 10, 0).await.unwrap();
    assert_eq!(tags.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn search_by_name(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);
    repo.create(TagCategory::General, "fursuit", true).await.unwrap();
    repo.create(TagCategory::General, "furry-art", true).await.unwrap();
    repo.create(TagCategory::General, "digital-art", true).await.unwrap();

    let results = repo.search_by_name("fur", 10).await.unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|t| t.name.starts_with("fur")));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn update_tag(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);
    let tag = repo.create(TagCategory::General, "old-name", false).await.unwrap();

    let updated = repo.update(tag.id, "new-name", true).await.unwrap();

    assert_eq!(updated.name, "new-name");
    assert!(updated.is_approved);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn increment_usage_count(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);
    let tag = repo.create(TagCategory::General, "inc-tag", true).await.unwrap();
    assert_eq!(tag.usage_count, 0);

    repo.increment_usage_count(tag.id).await.unwrap();
    repo.increment_usage_count(tag.id).await.unwrap();

    let found = repo.find_by_id(tag.id).await.unwrap().unwrap();
    assert_eq!(found.usage_count, 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn decrement_usage_count(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);
    let tag = repo.create(TagCategory::General, "dec-tag", true).await.unwrap();
    repo.increment_usage_count(tag.id).await.unwrap();
    repo.increment_usage_count(tag.id).await.unwrap();

    repo.decrement_usage_count(tag.id).await.unwrap();

    let found = repo.find_by_id(tag.id).await.unwrap().unwrap();
    assert_eq!(found.usage_count, 1);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn decrement_usage_count_floors_at_zero(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);
    let tag = repo.create(TagCategory::General, "floor-tag", true).await.unwrap();

    repo.decrement_usage_count(tag.id).await.unwrap();

    let found = repo.find_by_id(tag.id).await.unwrap().unwrap();
    assert_eq!(found.usage_count, 0, "usage_count should floor at 0");
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn delete_tag(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);
    let tag = repo.create(TagCategory::General, "delete-tag", true).await.unwrap();

    repo.delete(tag.id).await.unwrap();

    let found = repo.find_by_id(tag.id).await.unwrap();
    assert!(found.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn duplicate_name_category_fails(pool: PgPool) {
    let repo = SqlxTagRepository::new(pool);
    repo.create(TagCategory::General, "dup-tag", true).await.unwrap();

    let result = repo.create(TagCategory::General, "dup-tag", true).await;
    assert!(matches!(result, Err(TagError::NameTaken(_))));
}
