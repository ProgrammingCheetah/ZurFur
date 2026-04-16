//! Mock implementations for tag-related repository traits.

use std::sync::Arc;

use async_trait::async_trait;
use domain::entity::EntityKind;
use domain::entity_tag::{EntityTag, EntityTagError, EntityTagRepository};
use domain::tag::{Tag, TagCategory, TagError, TagRepository};
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct MockTagRepo {
    pub tags: Mutex<Vec<Tag>>,
    pub entity_tags: Arc<Mutex<Vec<EntityTag>>>,
}

impl MockTagRepo {
    pub fn new(entity_tags: Arc<Mutex<Vec<EntityTag>>>) -> Self {
        Self {
            tags: Mutex::new(vec![]),
            entity_tags,
        }
    }
}

#[async_trait]
impl TagRepository for MockTagRepo {
    async fn create(
        &self,
        category: TagCategory,
        name: &str,
        is_approved: bool,
    ) -> Result<Tag, TagError> {
        let mut tags = self.tags.lock().await;
        if tags.iter().any(|t| t.name == name && t.category == category) {
            return Err(TagError::NameTaken(name.into()));
        }
        let tag = Tag {
            id: Uuid::new_v4(),
            category,
            name: name.into(),
            usage_count: 0,
            is_approved,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        tags.push(tag.clone());
        Ok(tag)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Tag>, TagError> {
        Ok(self.tags.lock().await.iter().find(|t| t.id == id).cloned())
    }

    async fn find_by_name_and_category(
        &self,
        name: &str,
        category: TagCategory,
    ) -> Result<Option<Tag>, TagError> {
        Ok(self
            .tags
            .lock()
            .await
            .iter()
            .find(|t| t.name == name && t.category == category)
            .cloned())
    }

    async fn list_by_category(
        &self,
        category: TagCategory,
        limit: i64,
        _offset: i64,
    ) -> Result<Vec<Tag>, TagError> {
        Ok(self
            .tags
            .lock()
            .await
            .iter()
            .filter(|t| t.category == category)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn list_by_ids(&self, ids: &[Uuid]) -> Result<Vec<Tag>, TagError> {
        Ok(self
            .tags
            .lock()
            .await
            .iter()
            .filter(|t| ids.contains(&t.id))
            .cloned()
            .collect())
    }

    async fn search_by_name(&self, query: &str, limit: i64) -> Result<Vec<Tag>, TagError> {
        let q = query.to_lowercase();
        Ok(self
            .tags
            .lock()
            .await
            .iter()
            .filter(|t| t.name.starts_with(&q))
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn update(&self, id: Uuid, name: &str, is_approved: bool) -> Result<Tag, TagError> {
        let mut tags = self.tags.lock().await;
        let tag = tags.iter_mut().find(|t| t.id == id).ok_or(TagError::NotFound)?;
        tag.name = name.into();
        tag.is_approved = is_approved;
        Ok(tag.clone())
    }

    async fn increment_usage_count(&self, id: Uuid) -> Result<(), TagError> {
        let mut tags = self.tags.lock().await;
        if let Some(tag) = tags.iter_mut().find(|t| t.id == id) {
            tag.usage_count += 1;
        }
        Ok(())
    }

    async fn decrement_usage_count(&self, id: Uuid) -> Result<(), TagError> {
        let mut tags = self.tags.lock().await;
        if let Some(tag) = tags.iter_mut().find(|t| t.id == id) {
            tag.usage_count = (tag.usage_count - 1).max(0);
        }
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), TagError> {
        let mut tags = self.tags.lock().await;
        let len = tags.len();
        tags.retain(|t| t.id != id);
        if tags.len() == len {
            Err(TagError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn attach_and_increment(
        &self,
        entity_type: EntityKind,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<EntityTag, TagError> {
        let et = {
            let mut ets = self.entity_tags.lock().await;
            if ets.iter().any(|et| {
                et.entity_type == entity_type && et.entity_id == entity_id && et.tag_id == tag_id
            }) {
                return Err(TagError::AlreadyAttached);
            }
            let et = EntityTag { entity_type, entity_id, tag_id };
            ets.push(et.clone());
            et
        };
        self.increment_usage_count(tag_id).await?;
        Ok(et)
    }

    async fn detach_and_decrement(
        &self,
        entity_type: EntityKind,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<(), TagError> {
        {
            let mut ets = self.entity_tags.lock().await;
            let len = ets.len();
            ets.retain(|et| {
                !(et.entity_type == entity_type && et.entity_id == entity_id && et.tag_id == tag_id)
            });
            if ets.len() == len {
                return Err(TagError::NotAttached);
            }
        }
        self.decrement_usage_count(tag_id).await
    }

    async fn create_and_attach(
        &self,
        category: TagCategory,
        name: &str,
        is_approved: bool,
        _entity_type: EntityKind,
        _entity_id: Uuid,
    ) -> Result<Tag, TagError> {
        let mut tag = self.create(category, name, is_approved).await?;
        tag.usage_count = 1;
        let mut tags = self.tags.lock().await;
        if let Some(t) = tags.iter_mut().find(|t| t.id == tag.id) {
            t.usage_count = 1;
        }
        Ok(tag)
    }
}

pub struct MockEntityTagRepo {
    pub entity_tags: Arc<Mutex<Vec<EntityTag>>>,
}

impl MockEntityTagRepo {
    pub fn new(entity_tags: Arc<Mutex<Vec<EntityTag>>>) -> Self {
        Self { entity_tags }
    }
}

#[async_trait]
impl EntityTagRepository for MockEntityTagRepo {
    async fn attach(
        &self,
        entity_type: EntityKind,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<EntityTag, EntityTagError> {
        let mut ets = self.entity_tags.lock().await;
        if ets.iter().any(|et| {
            et.entity_type == entity_type && et.entity_id == entity_id && et.tag_id == tag_id
        }) {
            return Err(EntityTagError::AlreadyAttached);
        }
        let et = EntityTag {
            entity_type,
            entity_id,
            tag_id,
        };
        ets.push(et.clone());
        Ok(et)
    }

    async fn detach(
        &self,
        entity_type: EntityKind,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<(), EntityTagError> {
        let mut ets = self.entity_tags.lock().await;
        let len = ets.len();
        ets.retain(|et| {
            !(et.entity_type == entity_type && et.entity_id == entity_id && et.tag_id == tag_id)
        });
        if ets.len() == len {
            Err(EntityTagError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn list_by_entity(
        &self,
        entity_type: EntityKind,
        entity_id: Uuid,
    ) -> Result<Vec<EntityTag>, EntityTagError> {
        Ok(self
            .entity_tags
            .lock()
            .await
            .iter()
            .filter(|et| et.entity_type == entity_type && et.entity_id == entity_id)
            .cloned()
            .collect())
    }

    async fn list_by_tag(&self, tag_id: Uuid) -> Result<Vec<EntityTag>, EntityTagError> {
        Ok(self
            .entity_tags
            .lock()
            .await
            .iter()
            .filter(|et| et.tag_id == tag_id)
            .cloned()
            .collect())
    }
}
