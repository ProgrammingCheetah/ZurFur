use domain::default_role::DefaultRoleRepository;
use persistence::SqlxDefaultRoleRepository;
use sqlx::PgPool;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_default_roles(pool: PgPool) {
    let repo = SqlxDefaultRoleRepository::new(pool);

    let roles = repo.list_all().await.unwrap();

    assert_eq!(roles.len(), 4, "expected 4 seeded default roles");
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_by_name(pool: PgPool) {
    let repo = SqlxDefaultRoleRepository::new(pool);

    let owner = repo.find_by_name("owner").await.unwrap();
    assert!(owner.is_some());
    assert_eq!(owner.unwrap().name, "owner");

    let missing = repo.find_by_name("nonexistent").await.unwrap();
    assert!(missing.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn hierarchy_ordering(pool: PgPool) {
    let repo = SqlxDefaultRoleRepository::new(pool);
    let roles = repo.list_all().await.unwrap();

    // list_all orders by hierarchy_level ASC
    assert_eq!(roles[0].name, "owner");
    assert_eq!(roles[1].name, "admin");
    assert_eq!(roles[2].name, "mod");
    assert_eq!(roles[3].name, "member");
    assert!(roles[0].hierarchy_level < roles[3].hierarchy_level);
}
