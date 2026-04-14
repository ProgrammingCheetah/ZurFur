use std::sync::Arc;

use domain::entity_tag::{EntityTag, EntityTagRepository, TaggableEntityType};
use domain::tag::{Tag, TagCategory, TagRepository};
use uuid::Uuid;

/// Errors that can occur during tag operations.
#[derive(Debug, thiserror::Error)]
pub enum TagServiceError {
    #[error("Tag not found")]
    NotFound,
    #[error("Tag is not attached to this entity")]
    NotAttached,
    #[error("Tag name already taken: {0}")]
    NameTaken(String),
    #[error("Entity-backed tags cannot be modified or deleted")]
    Immutable,
    #[error("This category cannot be used for user-created tags")]
    InvalidCategory,
    #[error("Invalid tag name: {0}")]
    InvalidName(String),
    #[error("Tag is already attached to this entity")]
    AlreadyAttached,
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Application service for tag operations: CRUD, attach/detach, search, approval.
///
/// Enforces business rules:
/// - Organization/character tags are immutable (cannot update/delete)
/// - Tag names are trimmed, lowercased, and validated (1-100 chars)
/// - Usage count is maintained on attach/detach
/// - `create_entity_tag` uses transactional `create_and_attach`
pub struct TagService {
    tag_repo: Arc<dyn TagRepository>,
    entity_tag_repo: Arc<dyn EntityTagRepository>,
}

impl TagService {
    /// Create a new TagService with the given repository implementations.
    pub fn new(
        tag_repo: Arc<dyn TagRepository>,
        entity_tag_repo: Arc<dyn EntityTagRepository>,
    ) -> Self {
        Self {
            tag_repo,
            entity_tag_repo,
        }
    }

    /// Create a user-submitted tag. Only metadata and general categories allowed.
    pub async fn create_tag(
        &self,
        category: TagCategory,
        name: &str,
    ) -> Result<Tag, TagServiceError> {
        if category.is_immutable() {
            return Err(TagServiceError::InvalidCategory);
        }
        let name = Self::validate_tag_name(name)?;

        self.tag_repo
            .create(category, &name, false)
            .await
            .map_err(|e| match e {
                domain::tag::TagError::NameTaken(n) => TagServiceError::NameTaken(n),
                other => TagServiceError::Internal(other.to_string()),
            })
    }

    /// Create an entity-backed tag and attach it to the entity atomically.
    /// Auto-approved. Used for org/character tag auto-creation.
    pub async fn create_entity_tag(
        &self,
        category: TagCategory,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
        name: &str,
    ) -> Result<Tag, TagServiceError> {
        let name = Self::validate_tag_name(name)?;

        let tag = self
            .tag_repo
            .create_and_attach(category, &name, true, entity_type, entity_id)
            .await
            .map_err(|e| match e {
                domain::tag::TagError::NameTaken(n) => TagServiceError::NameTaken(n),
                other => TagServiceError::Internal(other.to_string()),
            })?;

        Ok(tag)
    }

    /// Look up a tag by UUID. Returns `NotFound` if it doesn't exist.
    pub async fn get_tag(&self, id: Uuid) -> Result<Tag, TagServiceError> {
        self.tag_repo
            .find_by_id(id)
            .await
            .map_err(|e| TagServiceError::Internal(e.to_string()))?
            .ok_or(TagServiceError::NotFound)
    }

    /// Search tags by name prefix (case-insensitive). Limit clamped to [1, 100].
    pub async fn search_tags(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Tag>, TagServiceError> {
        self.tag_repo
            .search_by_name(query, limit.clamp(1, 100))
            .await
            .map_err(|e| TagServiceError::Internal(e.to_string()))
    }

    /// List tags filtered by category, paginated. Limit clamped to [1, 100].
    pub async fn list_tags_by_category(
        &self,
        category: TagCategory,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Tag>, TagServiceError> {
        self.tag_repo
            .list_by_category(category, limit.clamp(1, 100), offset)
            .await
            .map_err(|e| TagServiceError::Internal(e.to_string()))
    }

    /// Update a tag's name and approval status. Rejects organization/character tags (immutable).
    pub async fn update_tag(
        &self,
        id: Uuid,
        name: &str,
        is_approved: bool,
    ) -> Result<Tag, TagServiceError> {
        let tag = self.get_tag(id).await?;
        Self::require_mutable(&tag)?;

        let name = Self::validate_tag_name(name)?;

        self.tag_repo
            .update(id, &name, is_approved)
            .await
            .map_err(|e| match e {
                domain::tag::TagError::NameTaken(n) => TagServiceError::NameTaken(n),
                domain::tag::TagError::NotFound => TagServiceError::NotFound,
                other => TagServiceError::Internal(other.to_string()),
            })
    }

    /// Hard-delete a tag. Rejects organization/character tags (immutable).
    pub async fn delete_tag(&self, id: Uuid) -> Result<(), TagServiceError> {
        let tag = self.get_tag(id).await?;
        Self::require_mutable(&tag)?;

        self.tag_repo
            .delete(id)
            .await
            .map_err(|e| match e {
                domain::tag::TagError::NotFound => TagServiceError::NotFound,
                other => TagServiceError::Internal(other.to_string()),
            })
    }

    /// Mark a tag as approved. Rejects organization/character tags (already auto-approved).
    pub async fn approve_tag(&self, id: Uuid) -> Result<Tag, TagServiceError> {
        let tag = self.get_tag(id).await?;
        Self::require_mutable(&tag)?;

        self.tag_repo
            .update(id, &tag.name, true)
            .await
            .map_err(|e| TagServiceError::Internal(e.to_string()))
    }

    /// Attach a tag to an entity and increment the tag's usage count.
    // TODO(Feature 3.5 Phase 2): attach + increment are not atomic — count can drift if increment fails. Needs UoW.
    pub async fn attach_tag(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<EntityTag, TagServiceError> {
        let entity_tag = self
            .entity_tag_repo
            .attach(entity_type, entity_id, tag_id)
            .await
            .map_err(|e| match e {
                domain::entity_tag::EntityTagError::AlreadyAttached => {
                    TagServiceError::AlreadyAttached
                }
                other => TagServiceError::Internal(other.to_string()),
            })?;

        self.tag_repo
            .increment_usage_count(tag_id)
            .await
            .map_err(|e| TagServiceError::Internal(e.to_string()))?;

        Ok(entity_tag)
    }

    /// Detach a tag from an entity and decrement the tag's usage count.
    // TODO(Feature 3.5 Phase 2): detach + decrement are not atomic — count can drift if decrement fails. Needs UoW.
    pub async fn detach_tag(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
        tag_id: Uuid,
    ) -> Result<(), TagServiceError> {
        self.entity_tag_repo
            .detach(entity_type, entity_id, tag_id)
            .await
            .map_err(|e| match e {
                domain::entity_tag::EntityTagError::NotFound => TagServiceError::NotAttached,
                other => TagServiceError::Internal(other.to_string()),
            })?;

        self.tag_repo
            .decrement_usage_count(tag_id)
            .await
            .map_err(|e| TagServiceError::Internal(e.to_string()))?;

        Ok(())
    }

    /// List all tags attached to an entity, returning full Tag objects.
    pub async fn list_tags_for_entity(
        &self,
        entity_type: TaggableEntityType,
        entity_id: Uuid,
    ) -> Result<Vec<Tag>, TagServiceError> {
        let entity_tags = self
            .entity_tag_repo
            .list_by_entity(entity_type, entity_id)
            .await
            .map_err(|e| TagServiceError::Internal(e.to_string()))?;

        if entity_tags.is_empty() {
            return Ok(vec![]);
        }

        let tag_ids: Vec<Uuid> = entity_tags.iter().map(|et| et.tag_id).collect();
        self.tag_repo
            .list_by_ids(&tag_ids)
            .await
            .map_err(|e| TagServiceError::Internal(e.to_string()))
    }

    /// Guard: rejects organization/character tags which are immutable identity markers.
    fn require_mutable(tag: &Tag) -> Result<(), TagServiceError> {
        if tag.category.is_immutable() {
            Err(TagServiceError::Immutable)
        } else {
            Ok(())
        }
    }

    /// Validate and normalize a tag name: trim whitespace, lowercase, check length (1-100).
    /// Returns the normalized name on success.
    fn validate_tag_name(name: &str) -> Result<String, TagServiceError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(TagServiceError::InvalidName("Tag name cannot be empty".into()));
        }
        let normalized = trimmed.to_lowercase();
        if normalized.chars().count() > 100 {
            return Err(TagServiceError::InvalidName(
                "Tag name cannot exceed 100 characters".into(),
            ));
        }
        Ok(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::entity_tag::{EntityTag, EntityTagError, EntityTagRepository, TaggableEntityType};
    use domain::tag::{Tag, TagCategory, TagError, TagRepository};
    use tokio::sync::Mutex;

    #[derive(Default)]
    struct MockTagRepo {
        tags: Mutex<Vec<Tag>>,
    }

    #[async_trait::async_trait]
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

        async fn create_and_attach(
            &self,
            category: TagCategory,
            name: &str,
            is_approved: bool,
            _entity_type: TaggableEntityType,
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

    #[derive(Default)]
    struct MockEntityTagRepo {
        entity_tags: Mutex<Vec<EntityTag>>,
    }

    #[async_trait::async_trait]
    impl EntityTagRepository for MockEntityTagRepo {
        async fn attach(
            &self,
            entity_type: TaggableEntityType,
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
            entity_type: TaggableEntityType,
            entity_id: Uuid,
            tag_id: Uuid,
        ) -> Result<(), EntityTagError> {
            let mut ets = self.entity_tags.lock().await;
            let len = ets.len();
            ets.retain(|et| {
                !(et.entity_type == entity_type
                    && et.entity_id == entity_id
                    && et.tag_id == tag_id)
            });
            if ets.len() == len {
                Err(EntityTagError::NotFound)
            } else {
                Ok(())
            }
        }

        async fn list_by_entity(
            &self,
            entity_type: TaggableEntityType,
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

    fn build_service() -> TagService {
        TagService::new(
            Arc::new(MockTagRepo::default()),
            Arc::new(MockEntityTagRepo::default()),
        )
    }

    #[tokio::test]
    async fn create_tag_succeeds() {
        let svc = build_service();
        let tag = svc.create_tag(TagCategory::Metadata, "canine").await.unwrap();
        assert_eq!(tag.name, "canine");
        assert_eq!(tag.category, TagCategory::Metadata);
        assert!(!tag.is_approved);
    }

    #[tokio::test]
    async fn create_tag_trims_and_lowercases() {
        let svc = build_service();
        let tag = svc.create_tag(TagCategory::General, "  Digital Art  ").await.unwrap();
        assert_eq!(tag.name, "digital art");
    }

    #[tokio::test]
    async fn create_tag_with_immutable_category_fails() {
        let svc = build_service();
        let err = svc.create_tag(TagCategory::Organization, "test").await.unwrap_err();
        assert!(matches!(err, TagServiceError::InvalidCategory));
    }

    #[tokio::test]
    async fn create_tag_empty_name_fails() {
        let svc = build_service();
        let err = svc.create_tag(TagCategory::General, "   ").await.unwrap_err();
        assert!(matches!(err, TagServiceError::InvalidName(_)));
    }

    #[tokio::test]
    async fn create_entity_tag_creates_and_attaches() {
        let svc = build_service();
        let org_id = Uuid::new_v4();

        let tag = svc
            .create_entity_tag(TagCategory::Organization, TaggableEntityType::Org, org_id, "my-studio")
            .await
            .unwrap();

        assert_eq!(tag.category, TagCategory::Organization);
        assert_eq!(tag.name, "my-studio");
        assert!(tag.is_approved);
        assert_eq!(tag.usage_count, 1);
    }

    #[tokio::test]
    async fn update_immutable_tag_fails() {
        let svc = build_service();
        let tag = svc
            .create_entity_tag(TagCategory::Organization, TaggableEntityType::Org, Uuid::new_v4(), "org-tag")
            .await
            .unwrap();

        let err = svc.update_tag(tag.id, "new-name", true).await.unwrap_err();
        assert!(matches!(err, TagServiceError::Immutable));
    }

    #[tokio::test]
    async fn delete_immutable_tag_fails() {
        let svc = build_service();
        let tag = svc
            .create_entity_tag(TagCategory::Character, TaggableEntityType::Character, Uuid::new_v4(), "foxy")
            .await
            .unwrap();

        let err = svc.delete_tag(tag.id).await.unwrap_err();
        assert!(matches!(err, TagServiceError::Immutable));
    }

    #[tokio::test]
    async fn attach_and_detach_tag() {
        let svc = build_service();
        let tag = svc.create_tag(TagCategory::Metadata, "canine").await.unwrap();
        assert_eq!(tag.usage_count, 0);
        let org_id = Uuid::new_v4();

        svc.attach_tag(TaggableEntityType::Org, org_id, tag.id).await.unwrap();

        let tags = svc.list_tags_for_entity(TaggableEntityType::Org, org_id).await.unwrap();
        assert_eq!(tags.len(), 1);

        let attached = svc.get_tag(tag.id).await.unwrap();
        assert_eq!(attached.usage_count, 1);

        svc.detach_tag(TaggableEntityType::Org, org_id, tag.id).await.unwrap();

        let tags = svc.list_tags_for_entity(TaggableEntityType::Org, org_id).await.unwrap();
        assert!(tags.is_empty());

        let detached = svc.get_tag(tag.id).await.unwrap();
        assert_eq!(detached.usage_count, 0);
    }

    #[tokio::test]
    async fn attach_tag_twice_fails() {
        let svc = build_service();
        let tag = svc.create_tag(TagCategory::Metadata, "canine").await.unwrap();
        let org_id = Uuid::new_v4();

        svc.attach_tag(TaggableEntityType::Org, org_id, tag.id).await.unwrap();

        let err = svc
            .attach_tag(TaggableEntityType::Org, org_id, tag.id)
            .await
            .unwrap_err();
        assert!(matches!(err, TagServiceError::AlreadyAttached));
    }

    #[tokio::test]
    async fn delete_mutable_tag_succeeds() {
        let svc = build_service();
        let tag = svc.create_tag(TagCategory::Metadata, "canine").await.unwrap();
        svc.delete_tag(tag.id).await.unwrap();
        let err = svc.get_tag(tag.id).await.unwrap_err();
        assert!(matches!(err, TagServiceError::NotFound));
    }

    #[tokio::test]
    async fn approve_tag_succeeds() {
        let svc = build_service();
        let tag = svc.create_tag(TagCategory::Metadata, "canine").await.unwrap();
        assert!(!tag.is_approved);

        let approved = svc.approve_tag(tag.id).await.unwrap();
        assert!(approved.is_approved);
    }
}
