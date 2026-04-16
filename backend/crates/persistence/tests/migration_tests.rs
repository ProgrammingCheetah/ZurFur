use sqlx::PgPool;

/// Verify all migrations apply cleanly to an empty database.
/// sqlx::test already runs migrations, so if this test runs, migrations work.
#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn migrations_run_cleanly(pool: PgPool) {
    // If we get here, all migrations applied without error.
    // Verify a table from the latest migration exists.
    let row = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM default_role")
        .fetch_one(&pool)
        .await
        .expect("default_role table should exist");

    // Seed data: 4 default roles (owner, admin, mod, member)
    assert_eq!(row, 4, "expected 4 seeded default roles");
}

/// Running migrations a second time should be idempotent.
#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn migrations_are_idempotent(pool: PgPool) {
    // sqlx::test already ran migrations once. Run them again.
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("re-running migrations should not fail");
}
