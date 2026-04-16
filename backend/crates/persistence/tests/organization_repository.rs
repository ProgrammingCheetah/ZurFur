mod common;

use common::*;
use domain::organization::{OrganizationError, OrganizationRepository};
use persistence::SqlxOrganizationRepository;
use sqlx::PgPool;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_org(pool: PgPool) {
    let repo = SqlxOrganizationRepository::new(pool);

    let org = repo.create("test-studio", Some("Test Studio"), false).await.unwrap();

    assert_eq!(org.slug, "test-studio");
    assert_eq!(org.display_name.as_deref(), Some("Test Studio"));
    assert!(!org.is_personal);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_org_by_id(pool: PgPool) {
    let repo = SqlxOrganizationRepository::new(pool);
    let org = repo.create("by-id-org", Some("By ID"), false).await.unwrap();

    let found = repo.find_by_id(org.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, org.id);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_org_by_slug(pool: PgPool) {
    let repo = SqlxOrganizationRepository::new(pool);
    repo.create("slug-lookup", Some("Slug Lookup"), false).await.unwrap();

    let found = repo.find_by_slug("slug-lookup").await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().slug, "slug-lookup");
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_personal_org(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let _org = create_test_org(&pool, "personal-org", None, true, Some(user.id)).await;

    let repo = SqlxOrganizationRepository::new(pool);
    let found = repo.find_personal_org(user.id).await.unwrap();
    assert!(found.is_some());
    assert!(found.unwrap().is_personal);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn update_display_name(pool: PgPool) {
    let repo = SqlxOrganizationRepository::new(pool);
    let org = repo.create("update-name-org", Some("Old Name"), false).await.unwrap();

    let updated = repo.update_display_name(org.id, Some("New Name")).await.unwrap();
    assert_eq!(updated.display_name.as_deref(), Some("New Name"));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn soft_delete_org(pool: PgPool) {
    let repo = SqlxOrganizationRepository::new(pool);
    let org = repo.create("delete-me-org", Some("Delete Me"), false).await.unwrap();

    repo.soft_delete(org.id).await.unwrap();

    let found = repo.find_by_id(org.id).await.unwrap();
    assert!(found.is_none(), "soft-deleted org should not be found");
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn duplicate_slug_fails(pool: PgPool) {
    let repo = SqlxOrganizationRepository::new(pool);
    repo.create("dup-slug", Some("First"), false).await.unwrap();

    let result = repo.create("dup-slug", Some("Second"), false).await;
    assert!(matches!(result, Err(OrganizationError::SlugTaken(_))));
}
