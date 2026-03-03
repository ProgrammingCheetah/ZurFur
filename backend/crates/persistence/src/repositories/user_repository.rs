use crate::pool::Pool;
use domain::user::{User, UserError, UserRepository};
use std::sync::Arc;
use uuid::Uuid;

/// SQLx implementation of `UserRepository`.
pub struct SqlxUserRepository {
    pool: Pool,
}

impl SqlxUserRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn from_pool(pool: Pool) -> Arc<dyn UserRepository> {
        Arc::new(Self::new(pool))
    }
}

#[async_trait::async_trait]
impl UserRepository for SqlxUserRepository {
    async fn find_by_email(&self, _email: &str) -> Result<Option<User>, UserError> {
        todo!()
    }

    async fn find_by_id(&self, _id: Uuid) -> Result<Option<User>, UserError> {
        todo!()
    }
}
