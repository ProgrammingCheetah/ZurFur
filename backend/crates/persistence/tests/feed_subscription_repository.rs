mod common;

use common::*;
use domain::feed_subscription::{FeedSubscriptionError, FeedSubscriptionRepository, SubscriptionPermission};
use persistence::SqlxFeedSubscriptionRepository;
use sqlx::PgPool;

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn create_subscription(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org = create_test_org(&pool, "sub-org", None, false, Some(user.id)).await;
    let feed = create_test_feed(&pool, "sub-feed", "Sub Feed", "custom", None).await;
    let repo = SqlxFeedSubscriptionRepository::new(pool);

    let sub = repo
        .create(feed.id, org.id, SubscriptionPermission::Read, user.id)
        .await
        .unwrap();

    assert_eq!(sub.feed_id, feed.id);
    assert_eq!(sub.subscriber_org_id, org.id);
    assert!(matches!(sub.permissions, SubscriptionPermission::Read));
    assert_eq!(sub.granted_by_user_id, user.id);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_feed(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org1 = create_test_org(&pool, "lbf-org-1", None, false, Some(user.id)).await;
    let org2 = create_test_org(&pool, "lbf-org-2", None, false, None).await;
    let feed = create_test_feed(&pool, "lbf-feed", "List Feed", "custom", None).await;
    let repo = SqlxFeedSubscriptionRepository::new(pool);

    repo.create(feed.id, org1.id, SubscriptionPermission::Read, user.id).await.unwrap();
    repo.create(feed.id, org2.id, SubscriptionPermission::ReadWrite, user.id).await.unwrap();

    let subs = repo.list_by_feed(feed.id).await.unwrap();
    assert_eq!(subs.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn list_by_subscriber(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org = create_test_org(&pool, "lbs-org", None, false, Some(user.id)).await;
    let f1 = create_test_feed(&pool, "lbs-feed-1", "Feed 1", "custom", None).await;
    let f2 = create_test_feed(&pool, "lbs-feed-2", "Feed 2", "custom", None).await;
    let repo = SqlxFeedSubscriptionRepository::new(pool);

    repo.create(f1.id, org.id, SubscriptionPermission::Read, user.id).await.unwrap();
    repo.create(f2.id, org.id, SubscriptionPermission::Admin, user.id).await.unwrap();

    let subs = repo.list_by_subscriber(org.id).await.unwrap();
    assert_eq!(subs.len(), 2);
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn update_permission(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org = create_test_org(&pool, "upd-perm-org", None, false, Some(user.id)).await;
    let feed = create_test_feed(&pool, "upd-perm-feed", "Perm Feed", "custom", None).await;
    let repo = SqlxFeedSubscriptionRepository::new(pool);

    let sub = repo
        .create(feed.id, org.id, SubscriptionPermission::Read, user.id)
        .await
        .unwrap();

    let updated = repo.update_permissions(sub.id, SubscriptionPermission::Admin).await.unwrap();
    assert!(matches!(updated.permissions, SubscriptionPermission::Admin));
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn delete_subscription(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org = create_test_org(&pool, "del-sub-org", None, false, Some(user.id)).await;
    let feed = create_test_feed(&pool, "del-sub-feed", "Del Feed", "custom", None).await;
    let repo = SqlxFeedSubscriptionRepository::new(pool);

    let sub = repo
        .create(feed.id, org.id, SubscriptionPermission::Read, user.id)
        .await
        .unwrap();

    repo.delete(sub.id).await.unwrap();

    let found = repo.find_by_id(sub.id).await.unwrap();
    assert!(found.is_none());
}

#[sqlx::test(migrator = "persistence::MIGRATOR")]
async fn duplicate_subscription_fails(pool: PgPool) {
    let user = create_test_user(&pool).await;
    let org = create_test_org(&pool, "dup-sub-org", None, false, Some(user.id)).await;
    let feed = create_test_feed(&pool, "dup-sub-feed", "Dup Feed", "custom", None).await;
    let repo = SqlxFeedSubscriptionRepository::new(pool);

    repo.create(feed.id, org.id, SubscriptionPermission::Read, user.id).await.unwrap();

    let result = repo
        .create(feed.id, org.id, SubscriptionPermission::ReadWrite, user.id)
        .await;

    assert!(matches!(result, Err(FeedSubscriptionError::AlreadySubscribed)));
}
