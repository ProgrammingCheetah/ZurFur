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
async fn create_org_with_owner(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let repo = SqlxOrganizationRepository::new(pool.clone());

    let org = repo
        .create_with_owner("owned-org", Some("Owned Org"), false, user.id)
        .await
        .unwrap();

    assert_eq!(org.slug, "owned-org");

    // Verify owner membership was also created
    let member = sqlx::query_as::<_, (String, i64)>(
        "SELECT role, permissions FROM organization_member WHERE org_id = $1 AND user_id = $2",
    )
    .bind(org.id)
    .bind(user.id)
    .fetch_one(&pool)
    .await
    .expect("owner member should exist");

    assert_eq!(member.0, "owner");
    assert_eq!(member.1, -1, "owner should have ALL permissions (i64 -1 = u64::MAX)");
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_org_with_owner_rollback_on_invalid_user(pool: PgPool) {
    let repo = SqlxOrganizationRepository::new(pool.clone());

    // Non-existent user_id causes FK violation on organization_member.user_id,
    // which should roll back the entire transaction including the org creation.
    let fake_user_id = uuid::Uuid::new_v4();
    let result = repo
        .create_with_owner("rollback-org", Some("Rollback Org"), false, fake_user_id)
        .await;

    assert!(result.is_err(), "create_with_owner should fail for non-existent user");

    // Verify the org was NOT created (transaction rolled back)
    let org_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM organization WHERE slug = 'rollback-org'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(org_exists, 0, "org should not exist after rollback");
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn duplicate_slug_fails(pool: PgPool) {
    let repo = SqlxOrganizationRepository::new(pool);
    repo.create("dup-slug", Some("First"), false).await.unwrap();

    let result = repo.create("dup-slug", Some("Second"), false).await;
    assert!(matches!(result, Err(OrganizationError::SlugTaken(_))));
}
