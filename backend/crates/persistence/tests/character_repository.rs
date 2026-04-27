mod common;

use common::*;
use domain::character::{CharacterRepository, CharacterVisibility};
use domain::content_rating::ContentRating;
use persistence::SqlxCharacterRepository;
use sqlx::PgPool;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_character(pool: PgPool) {
    let org = create_test_org(&pool, "char-org", Some("Char Org"), false, None).await;
    let repo = SqlxCharacterRepository::new(pool);

    let character = repo
        .create(
            org.id,
            "Foxy",
            Some("A friendly fox"),
            ContentRating::Sfw,
            CharacterVisibility::Public,
        )
        .await
        .unwrap();

    assert_eq!(character.org_id, org.id);
    assert_eq!(character.name, "Foxy");
    assert_eq!(character.description.as_deref(), Some("A friendly fox"));
    assert_eq!(character.content_rating, ContentRating::Sfw);
    assert_eq!(character.visibility, CharacterVisibility::Public);
    assert!(character.deleted_at.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_character_by_id(pool: PgPool) {
    let org = create_test_org(&pool, "find-org", None, false, None).await;
    let repo = SqlxCharacterRepository::new(pool);

    let created = repo
        .create(org.id, "Luna", None, ContentRating::Sfw, CharacterVisibility::Public)
        .await
        .unwrap();

    let found = repo.find_by_id(created.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, created.id);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn find_deleted_character_returns_none(pool: PgPool) {
    let org = create_test_org(&pool, "del-find-org", None, false, None).await;
    let repo = SqlxCharacterRepository::new(pool);

    let created = repo
        .create(org.id, "Ghost", None, ContentRating::Sfw, CharacterVisibility::Public)
        .await
        .unwrap();

    repo.soft_delete(created.id).await.unwrap();

    let found = repo.find_by_id(created.id).await.unwrap();
    assert!(found.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_org_basic(pool: PgPool) {
    let org = create_test_org(&pool, "list-org", None, false, None).await;
    let repo = SqlxCharacterRepository::new(pool);

    for name in ["Alpha", "Beta", "Gamma"] {
        repo.create(org.id, name, None, ContentRating::Sfw, CharacterVisibility::Public)
            .await
            .unwrap();
    }

    let list = repo.list_by_org(org.id, 10, 0, None).await.unwrap();
    assert_eq!(list.len(), 3);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_org_excludes_deleted(pool: PgPool) {
    let org = create_test_org(&pool, "list-del-org", None, false, None).await;
    let repo = SqlxCharacterRepository::new(pool);

    let c1 = repo
        .create(org.id, "Alive", None, ContentRating::Sfw, CharacterVisibility::Public)
        .await
        .unwrap();
    let c2 = repo
        .create(org.id, "Deleted", None, ContentRating::Sfw, CharacterVisibility::Public)
        .await
        .unwrap();

    repo.soft_delete(c2.id).await.unwrap();

    let list = repo.list_by_org(org.id, 10, 0, None).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, c1.id);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_org_filter_content_rating(pool: PgPool) {
    let org = create_test_org(&pool, "filter-cr-org", None, false, None).await;
    let repo = SqlxCharacterRepository::new(pool);

    repo.create(org.id, "SFW Char", None, ContentRating::Sfw, CharacterVisibility::Public)
        .await
        .unwrap();
    repo.create(org.id, "NSFW Char", None, ContentRating::Nsfw, CharacterVisibility::Public)
        .await
        .unwrap();

    let sfw_only = repo
        .list_by_org(org.id, 10, 0, Some(ContentRating::Sfw))
        .await
        .unwrap();
    assert_eq!(sfw_only.len(), 1);
    assert_eq!(sfw_only[0].name, "SFW Char");
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_org_pagination(pool: PgPool) {
    let org = create_test_org(&pool, "page-org", None, false, None).await;
    let repo = SqlxCharacterRepository::new(pool);

    // Create 3 characters — they'll be ordered by created_at DESC
    for name in ["First", "Second", "Third"] {
        repo.create(org.id, name, None, ContentRating::Sfw, CharacterVisibility::Public)
            .await
            .unwrap();
    }

    let page = repo.list_by_org(org.id, 2, 1, None).await.unwrap();
    assert_eq!(page.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn update_character(pool: PgPool) {
    let org = create_test_org(&pool, "update-org", None, false, None).await;
    let repo = SqlxCharacterRepository::new(pool);

    let created = repo
        .create(
            org.id,
            "Old Name",
            Some("Old desc"),
            ContentRating::Sfw,
            CharacterVisibility::Public,
        )
        .await
        .unwrap();

    let updated = repo
        .update(
            created.id,
            "New Name",
            Some("New desc"),
            ContentRating::Questionable,
            CharacterVisibility::Private,
        )
        .await
        .unwrap();

    assert_eq!(updated.name, "New Name");
    assert_eq!(updated.description.as_deref(), Some("New desc"));
    assert_eq!(updated.content_rating, ContentRating::Questionable);
    assert_eq!(updated.visibility, CharacterVisibility::Private);
    assert!(updated.updated_at > created.updated_at);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn soft_delete_character(pool: PgPool) {
    let org = create_test_org(&pool, "softdel-org", None, false, None).await;
    let repo = SqlxCharacterRepository::new(pool);

    let created = repo
        .create(org.id, "Doomed", None, ContentRating::Sfw, CharacterVisibility::Public)
        .await
        .unwrap();

    repo.soft_delete(created.id).await.unwrap();

    let found = repo.find_by_id(created.id).await.unwrap();
    assert!(found.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn character_requires_valid_org(pool: PgPool) {
    let repo = SqlxCharacterRepository::new(pool);
    let fake_org_id = uuid::Uuid::new_v4();

    let result = repo
        .create(
            fake_org_id,
            "Orphan",
            None,
            ContentRating::Sfw,
            CharacterVisibility::Public,
        )
        .await;

    assert!(result.is_err());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn character_content_rating_enum(pool: PgPool) {
    let org = create_test_org(&pool, "cr-enum-org", None, false, None).await;
    let repo = SqlxCharacterRepository::new(pool);

    for (rating, name) in [
        (ContentRating::Sfw, "SFW"),
        (ContentRating::Questionable, "Questionable"),
        (ContentRating::Nsfw, "NSFW"),
    ] {
        let c = repo
            .create(org.id, name, None, rating, CharacterVisibility::Public)
            .await
            .unwrap();
        assert_eq!(c.content_rating, rating);
    }
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn character_visibility_enum(pool: PgPool) {
    let org = create_test_org(&pool, "vis-enum-org", None, false, None).await;
    let repo = SqlxCharacterRepository::new(pool);

    for (visibility, name) in [
        (CharacterVisibility::Public, "Pub"),
        (CharacterVisibility::Private, "Priv"),
        (CharacterVisibility::Controlled, "Ctrl"),
        (CharacterVisibility::Unlisted, "Unl"),
    ] {
        let c = repo
            .create(org.id, name, None, ContentRating::Sfw, visibility)
            .await
            .unwrap();
        assert_eq!(c.visibility, visibility);
    }
}
