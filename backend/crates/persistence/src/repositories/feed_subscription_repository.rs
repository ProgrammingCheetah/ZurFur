use crate::pool::Pool;
use crate::sqlx_utils::is_unique_violation;
use domain::feed_subscription::{
    FeedSubscription, FeedSubscriptionError, FeedSubscriptionRepository, SubscriptionPermission,
};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub struct SqlxFeedSubscriptionRepository {
    pool: Pool,
}

impl SqlxFeedSubscriptionRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn from_pool(pool: Pool) -> Arc<dyn FeedSubscriptionRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_subscription(row: sqlx::postgres::PgRow) -> Result<FeedSubscription, FeedSubscriptionError> {
    let perms_str: String = row.get("permissions");
    let permissions = SubscriptionPermission::from_str(&perms_str)
        .ok_or_else(|| FeedSubscriptionError::Database(format!("Unknown permission: {perms_str}")))?;

    let sub = FeedSubscription {
        id: row.get("id"),
        feed_id: row.get("feed_id"),
        subscriber_org_id: row.get("subscriber_org_id"),
        permissions,
        granted_at: row.get("granted_at"),
        granted_by_user_id: row.get("granted_by_user_id"),
    };
    Ok(sub)
}

#[async_trait::async_trait]
impl FeedSubscriptionRepository for SqlxFeedSubscriptionRepository {
    async fn create(
        &self,
        feed_id: Uuid,
        subscriber_org_id: Uuid,
        permissions: SubscriptionPermission,
        granted_by_user_id: Uuid,
    ) -> Result<FeedSubscription, FeedSubscriptionError> {
        let row = sqlx::query(
            "INSERT INTO feed_subscriptions (feed_id, subscriber_org_id, permissions, granted_by_user_id) \
             VALUES ($1, $2, $3, $4) \
             RETURNING id, feed_id, subscriber_org_id, permissions, granted_at, granted_by_user_id",
        )
        .bind(feed_id)
        .bind(subscriber_org_id)
        .bind(permissions.as_str())
        .bind(granted_by_user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                FeedSubscriptionError::AlreadySubscribed
            } else {
                FeedSubscriptionError::Database(e.to_string())
            }
        })?;

        map_subscription(row)
    }

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<FeedSubscription>, FeedSubscriptionError> {
        let row = sqlx::query(
            "SELECT id, feed_id, subscriber_org_id, permissions, granted_at, granted_by_user_id \
             FROM feed_subscriptions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| FeedSubscriptionError::Database(e.to_string()))?;

        match row {
            Some(r) => Ok(Some(map_subscription(r)?)),
            None => Ok(None),
        }
    }

    async fn list_by_feed(
        &self,
        feed_id: Uuid,
    ) -> Result<Vec<FeedSubscription>, FeedSubscriptionError> {
        let rows = sqlx::query(
            "SELECT id, feed_id, subscriber_org_id, permissions, granted_at, granted_by_user_id \
             FROM feed_subscriptions WHERE feed_id = $1",
        )
        .bind(feed_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| FeedSubscriptionError::Database(e.to_string()))?;

        let mut subs = Vec::with_capacity(rows.len());
        for row in rows {
            subs.push(map_subscription(row)?);
        }
        Ok(subs)
    }

    async fn list_by_subscriber(
        &self,
        subscriber_org_id: Uuid,
    ) -> Result<Vec<FeedSubscription>, FeedSubscriptionError> {
        let rows = sqlx::query(
            "SELECT id, feed_id, subscriber_org_id, permissions, granted_at, granted_by_user_id \
             FROM feed_subscriptions WHERE subscriber_org_id = $1",
        )
        .bind(subscriber_org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| FeedSubscriptionError::Database(e.to_string()))?;

        let mut subs = Vec::with_capacity(rows.len());
        for row in rows {
            subs.push(map_subscription(row)?);
        }
        Ok(subs)
    }

    async fn update_permissions(
        &self,
        id: Uuid,
        permissions: SubscriptionPermission,
    ) -> Result<FeedSubscription, FeedSubscriptionError> {
        let row = sqlx::query(
            "UPDATE feed_subscriptions SET permissions = $1 \
             WHERE id = $2 \
             RETURNING id, feed_id, subscriber_org_id, permissions, granted_at, granted_by_user_id",
        )
        .bind(permissions.as_str())
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| FeedSubscriptionError::Database(e.to_string()))?
        .ok_or(FeedSubscriptionError::NotFound)?;

        map_subscription(row)
    }

    async fn delete(&self, id: Uuid) -> Result<(), FeedSubscriptionError> {
        let result = sqlx::query("DELETE FROM feed_subscriptions WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| FeedSubscriptionError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(FeedSubscriptionError::NotFound);
        }
        Ok(())
    }
}
