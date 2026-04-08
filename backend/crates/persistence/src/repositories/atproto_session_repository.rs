use crate::pool::Pool;
use domain::atproto_session::{AtprotoSessionEntity, AtprotoSessionRepository};
use domain::user::UserError;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub struct SqlxAtprotoSessionRepository {
    pool: Pool,
}

impl SqlxAtprotoSessionRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn from_pool(pool: Pool) -> Arc<dyn AtprotoSessionRepository> {
        Arc::new(Self::new(pool))
    }
}

fn map_session(row: sqlx::postgres::PgRow) -> AtprotoSessionEntity {
    AtprotoSessionEntity {
        id: row.get("id"),
        user_id: row.get("user_id"),
        did: row.get("did"),
        access_token: row.get("access_token"),
        refresh_token: row.get("refresh_token"),
        expires_at: row.get("expires_at"),
        pds_url: row.get("pds_url"),
    }
}

#[async_trait::async_trait]
impl AtprotoSessionRepository for SqlxAtprotoSessionRepository {
    async fn upsert(&self, session: &AtprotoSessionEntity) -> Result<(), UserError> {
        sqlx::query(
            r#"INSERT INTO atproto_sessions (id, user_id, did, access_token, refresh_token, expires_at, pds_url)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               ON CONFLICT (user_id) DO UPDATE SET
                   did = EXCLUDED.did,
                   access_token = EXCLUDED.access_token,
                   refresh_token = EXCLUDED.refresh_token,
                   expires_at = EXCLUDED.expires_at,
                   pds_url = EXCLUDED.pds_url"#,
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(&session.did)
        .bind(&session.access_token)
        .bind(&session.refresh_token)
        .bind(session.expires_at)
        .bind(&session.pds_url)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| UserError::Database(e.to_string()))
    }

    async fn find_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<Option<AtprotoSessionEntity>, UserError> {
        sqlx::query("SELECT id, user_id, did, access_token, refresh_token, expires_at, pds_url FROM atproto_sessions WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map(|opt| opt.map(map_session))
            .map_err(|e| UserError::Database(e.to_string()))
    }

    async fn find_by_did(&self, did: &str) -> Result<Option<AtprotoSessionEntity>, UserError> {
        sqlx::query("SELECT id, user_id, did, access_token, refresh_token, expires_at, pds_url FROM atproto_sessions WHERE did = $1")
            .bind(did)
            .fetch_optional(&self.pool)
            .await
            .map(|opt| opt.map(map_session))
            .map_err(|e| UserError::Database(e.to_string()))
    }

    async fn delete_by_user_id(&self, user_id: Uuid) -> Result<(), UserError> {
        sqlx::query("DELETE FROM atproto_sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| UserError::Database(e.to_string()))
    }
}
