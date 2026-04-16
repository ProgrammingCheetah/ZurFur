mod common;

use common::*;
use domain::user_preferences::UserPreferencesRepository;
use persistence::SqlxUserPreferencesRepository;
use sqlx::PgPool;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn get_default_preferences(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let repo = SqlxUserPreferencesRepository::new(pool);

    let prefs = repo.get(user.id).await.unwrap();

    assert_eq!(prefs.user_id, user.id);
    assert_eq!(prefs.settings, "{}");
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn set_and_get_preferences(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let repo = SqlxUserPreferencesRepository::new(pool);

    let settings = r#"{"max_content_rating":"sfw","theme":"dark"}"#;
    repo.set(user.id, settings).await.unwrap();

    let prefs = repo.get(user.id).await.unwrap();
    assert!(prefs.settings.contains("dark"));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn update_preferences(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let repo = SqlxUserPreferencesRepository::new(pool);

    repo.set(user.id, r#"{"theme":"light"}"#).await.unwrap();
    repo.set(user.id, r#"{"theme":"dark"}"#).await.unwrap();

    let prefs = repo.get(user.id).await.unwrap();
    assert!(prefs.settings.contains("dark"));
    assert!(!prefs.settings.contains("light"));
}
