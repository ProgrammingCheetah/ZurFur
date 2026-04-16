mod common;

use common::*;
use domain::organization_member::{OrganizationMemberError, OrganizationMemberRepository, Permissions, Role};
use persistence::SqlxOrganizationMemberRepository;
use sqlx::PgPool;

async fn setup(pool: &PgPool) -> (TestUser, TestOrg) {
    let user = create_test_user(pool).await;
    let org = create_test_org(pool, &format!("org-{}", &user.id.to_string()[..8]), None, false, None).await;
    (user, org)
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn add_member(pool: PgPool) {
    let (user, org) = setup(&pool).await;
    let repo = SqlxOrganizationMemberRepository::new(pool);

    let member = repo
        .add(org.id, user.id, Role::Member, Some("Artist"), Permissions::new(0))
        .await
        .unwrap();

    assert_eq!(member.org_id, org.id);
    assert_eq!(member.user_id, user.id);
    assert_eq!(member.role, Role::Member);
    assert_eq!(member.title.as_deref(), Some("Artist"));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_by_org_and_user(pool: PgPool) {
    let (user, org) = setup(&pool).await;
    let repo = SqlxOrganizationMemberRepository::new(pool);
    repo.add(org.id, user.id, Role::Admin, None, Permissions::new(0)).await.unwrap();

    let found = repo.find_by_org_and_user(org.id, user.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().role, Role::Admin);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_org(pool: PgPool) {
    let user1 = create_test_user(&pool).await;
    let user2 = create_test_user(&pool).await;
    let org = create_test_org(&pool, "list-org", None, false, None).await;
    let repo = SqlxOrganizationMemberRepository::new(pool);

    repo.add(org.id, user1.id, Role::Owner, None, Permissions::new(Permissions::ALL)).await.unwrap();
    repo.add(org.id, user2.id, Role::Member, None, Permissions::new(0)).await.unwrap();

    let members = repo.list_by_org(org.id).await.unwrap();
    assert_eq!(members.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_user(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org1 = create_test_org(&pool, "user-org-1", None, false, None).await;
    let org2 = create_test_org(&pool, "user-org-2", None, false, None).await;
    let repo = SqlxOrganizationMemberRepository::new(pool);

    repo.add(org1.id, user.id, Role::Owner, None, Permissions::new(Permissions::ALL)).await.unwrap();
    repo.add(org2.id, user.id, Role::Member, None, Permissions::new(0)).await.unwrap();

    let memberships = repo.list_by_user(user.id).await.unwrap();
    assert_eq!(memberships.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn update_role_and_title(pool: PgPool) {
    let (user, org) = setup(&pool).await;
    let repo = SqlxOrganizationMemberRepository::new(pool);
    repo.add(org.id, user.id, Role::Member, None, Permissions::new(0)).await.unwrap();

    let updated = repo
        .update_role_and_title(org.id, user.id, Role::Admin, Some("Lead Artist"))
        .await
        .unwrap();

    assert_eq!(updated.role, Role::Admin);
    assert_eq!(updated.title.as_deref(), Some("Lead Artist"));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn update_permissions(pool: PgPool) {
    let (user, org) = setup(&pool).await;
    let repo = SqlxOrganizationMemberRepository::new(pool);
    repo.add(org.id, user.id, Role::Member, None, Permissions::new(0)).await.unwrap();

    let new_perms = Permissions::new(Permissions::MANAGE_PROFILE | Permissions::CHAT);
    let updated = repo.update_permissions(org.id, user.id, new_perms).await.unwrap();

    assert!(updated.permissions.has(Permissions::MANAGE_PROFILE));
    assert!(updated.permissions.has(Permissions::CHAT));
    assert!(!updated.permissions.has(Permissions::MANAGE_MEMBERS));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn remove_member(pool: PgPool) {
    let (user, org) = setup(&pool).await;
    let repo = SqlxOrganizationMemberRepository::new(pool);
    repo.add(org.id, user.id, Role::Member, None, Permissions::new(0)).await.unwrap();

    repo.remove(org.id, user.id).await.unwrap();

    let found = repo.find_by_org_and_user(org.id, user.id).await.unwrap();
    assert!(found.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn add_duplicate_member_fails(pool: PgPool) {
    let (user, org) = setup(&pool).await;
    let repo = SqlxOrganizationMemberRepository::new(pool);
    repo.add(org.id, user.id, Role::Member, None, Permissions::new(0)).await.unwrap();

    let result = repo
        .add(org.id, user.id, Role::Admin, None, Permissions::new(0))
        .await;

    assert!(matches!(result, Err(OrganizationMemberError::AlreadyMember)));
}
