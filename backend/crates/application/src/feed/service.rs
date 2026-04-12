use std::sync::Arc;

use domain::entity_feed::{EntityFeedRepository, EntityType};
use domain::feed::{Feed, FeedError, FeedRepository, FeedType};
use domain::feed_element::{FeedElement, FeedElementRepository, FeedElementType};
use domain::feed_item::{AuthorType, FeedItem, FeedItemRepository};
use domain::organization_member::{OrganizationMemberRepository, Permissions};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum FeedServiceError {
    #[error("Feed not found")]
    FeedNotFound,
    #[error("Feed item not found")]
    ItemNotFound,
    #[error("System feeds cannot be deleted")]
    SystemFeedUndeletable,
    #[error("Permission denied")]
    Forbidden,
    #[error("Feed slug already taken: {0}")]
    SlugTaken(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

pub struct NewFeedElement {
    pub element_type: FeedElementType,
    pub content_json: String,
    pub position: i32,
}

pub struct FeedItemWithElements {
    pub item: FeedItem,
    pub elements: Vec<FeedElement>,
}

pub struct FeedService {
    feed_repo: Arc<dyn FeedRepository>,
    entity_feed_repo: Arc<dyn EntityFeedRepository>,
    feed_item_repo: Arc<dyn FeedItemRepository>,
    feed_element_repo: Arc<dyn FeedElementRepository>,
    member_repo: Arc<dyn OrganizationMemberRepository>,
}

impl FeedService {
    pub fn new(
        feed_repo: Arc<dyn FeedRepository>,
        entity_feed_repo: Arc<dyn EntityFeedRepository>,
        feed_item_repo: Arc<dyn FeedItemRepository>,
        feed_element_repo: Arc<dyn FeedElementRepository>,
        member_repo: Arc<dyn OrganizationMemberRepository>,
    ) -> Self {
        Self {
            feed_repo,
            entity_feed_repo,
            feed_item_repo,
            feed_element_repo,
            member_repo,
        }
    }

    /// List all feeds attached to an entity.
    pub async fn list_feeds_for_entity(
        &self,
        entity_type: EntityType,
        entity_id: Uuid,
    ) -> Result<Vec<Feed>, FeedServiceError> {
        let entity_feeds = self
            .entity_feed_repo
            .list_by_entity(entity_type, entity_id)
            .await
            .map_err(|e| FeedServiceError::Internal(e.to_string()))?;

        if entity_feeds.is_empty() {
            return Ok(vec![]);
        }

        let feed_ids: Vec<Uuid> = entity_feeds.iter().map(|ef| ef.feed_id).collect();
        let feeds = self
            .feed_repo
            .list_by_ids(&feed_ids)
            .await
            .map_err(|e| FeedServiceError::Internal(e.to_string()))?;

        Ok(feeds)
    }

    /// Get a feed by ID.
    pub async fn get_feed(&self, feed_id: Uuid) -> Result<Feed, FeedServiceError> {
        self.feed_repo
            .find_by_id(feed_id)
            .await
            .map_err(|e| FeedServiceError::Internal(e.to_string()))?
            .ok_or(FeedServiceError::FeedNotFound)
    }

    /// Create a system feed and attach it to an org. No permission check —
    /// called from orchestration layer (org creation, onboarding), not from users.
    pub async fn create_system_feed(
        &self,
        org_id: Uuid,
        slug: &str,
        display_name: &str,
    ) -> Result<Feed, FeedServiceError> {
        let feed = self
            .feed_repo
            .create(slug, display_name, None, FeedType::System)
            .await
            .map_err(|e| match e {
                FeedError::SlugTaken(s) => FeedServiceError::SlugTaken(s),
                other => FeedServiceError::Internal(other.to_string()),
            })?;

        self.entity_feed_repo
            .attach(feed.id, EntityType::Org, org_id)
            .await
            .map_err(|e| FeedServiceError::Internal(e.to_string()))?;

        Ok(feed)
    }

    /// Create a custom feed and attach it to an org.
    pub async fn create_custom_feed(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
        slug: &str,
        display_name: &str,
        description: Option<&str>,
    ) -> Result<Feed, FeedServiceError> {
        self.require_feed_permission_on_org(org_id, actor_id, Permissions::MANAGE_PROFILE)
            .await?;

        let feed = self
            .feed_repo
            .create(slug, display_name, description, FeedType::Custom)
            .await
            .map_err(|e| match e {
                FeedError::SlugTaken(s) => FeedServiceError::SlugTaken(s),
                other => FeedServiceError::Internal(other.to_string()),
            })?;

        self.entity_feed_repo
            .attach(feed.id, EntityType::Org, org_id)
            .await
            .map_err(|e| FeedServiceError::Internal(e.to_string()))?;

        Ok(feed)
    }

    /// Update a feed's display name and description.
    pub async fn update_feed(
        &self,
        feed_id: Uuid,
        actor_id: Uuid,
        display_name: &str,
        description: Option<&str>,
    ) -> Result<Feed, FeedServiceError> {
        let org_id = self.resolve_feed_org(feed_id).await?;
        self.require_feed_permission_on_org(org_id, actor_id, Permissions::MANAGE_PROFILE)
            .await?;

        self.feed_repo
            .update(feed_id, display_name, description)
            .await
            .map_err(|e| match e {
                FeedError::NotFound => FeedServiceError::FeedNotFound,
                other => FeedServiceError::Internal(other.to_string()),
            })
    }

    /// Soft-delete a feed. System feeds cannot be deleted.
    pub async fn delete_feed(
        &self,
        feed_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), FeedServiceError> {
        let org_id = self.resolve_feed_org(feed_id).await?;
        self.require_feed_permission_on_org(org_id, actor_id, Permissions::MANAGE_PROFILE)
            .await?;

        self.feed_repo.soft_delete(feed_id).await.map_err(|e| match e {
            FeedError::SystemFeedUndeletable => FeedServiceError::SystemFeedUndeletable,
            FeedError::NotFound => FeedServiceError::FeedNotFound,
            other => FeedServiceError::Internal(other.to_string()),
        })
    }

    /// Post a new item with elements to a feed.
    pub async fn post_to_feed(
        &self,
        feed_id: Uuid,
        actor_id: Uuid,
        elements: Vec<NewFeedElement>,
    ) -> Result<FeedItemWithElements, FeedServiceError> {
        let org_id = self.resolve_feed_org(feed_id).await?;
        self.require_feed_permission_on_org(org_id, actor_id, Permissions::MANAGE_PROFILE)
            .await?;

        let item = self
            .feed_item_repo
            .create(feed_id, AuthorType::User, actor_id)
            .await
            .map_err(|e| FeedServiceError::Internal(e.to_string()))?;

        let mut created_elements = Vec::with_capacity(elements.len());
        for el in elements {
            let element = self
                .feed_element_repo
                .create(item.id, el.element_type, &el.content_json, el.position)
                .await
                .map_err(|e| FeedServiceError::Internal(e.to_string()))?;
            created_elements.push(element);
        }

        Ok(FeedItemWithElements {
            item,
            elements: created_elements,
        })
    }

    /// List feed items with their elements, paginated.
    pub async fn list_feed_items(
        &self,
        feed_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<FeedItemWithElements>, FeedServiceError> {
        let items = self
            .feed_item_repo
            .list_by_feed(feed_id, limit, offset)
            .await
            .map_err(|e| FeedServiceError::Internal(e.to_string()))?;

        let mut results = Vec::with_capacity(items.len());
        for item in items {
            let elements = self
                .feed_element_repo
                .list_by_feed_item(item.id)
                .await
                .map_err(|e| FeedServiceError::Internal(e.to_string()))?;
            results.push(FeedItemWithElements { item, elements });
        }

        Ok(results)
    }

    /// Delete a feed item. Only the author or an org admin can delete.
    pub async fn delete_feed_item(
        &self,
        item_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), FeedServiceError> {
        let item = self
            .feed_item_repo
            .find_by_id(item_id)
            .await
            .map_err(|e| FeedServiceError::Internal(e.to_string()))?
            .ok_or(FeedServiceError::ItemNotFound)?;

        // Author can always delete their own items
        if item.author_id != actor_id {
            // Otherwise, check org permissions
            let org_id = self.resolve_feed_org(item.feed_id).await?;
            self.require_feed_permission_on_org(org_id, actor_id, Permissions::MANAGE_PROFILE)
                .await?;
        }

        self.feed_item_repo
            .delete(item_id)
            .await
            .map_err(|e| FeedServiceError::Internal(e.to_string()))
    }

    // --- Private helpers --------------------------------------------------------

    /// Look up the owning org for a feed via entity_feeds.
    async fn resolve_feed_org(&self, feed_id: Uuid) -> Result<Uuid, FeedServiceError> {
        let entity_feed = self
            .entity_feed_repo
            .find_by_feed_id(feed_id)
            .await
            .map_err(|e| FeedServiceError::Internal(e.to_string()))?
            .ok_or(FeedServiceError::FeedNotFound)?;

        // For now, we only support org-owned feeds for permission checks
        if entity_feed.entity_type != EntityType::Org {
            return Err(FeedServiceError::Forbidden);
        }

        Ok(entity_feed.entity_id)
    }

    /// Check that the actor has the given permission on the org.
    async fn require_feed_permission_on_org(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
        permission: u64,
    ) -> Result<(), FeedServiceError> {
        let member = self
            .member_repo
            .find_by_org_and_user(org_id, actor_id)
            .await
            .map_err(|e| FeedServiceError::Internal(e.to_string()))?
            .ok_or(FeedServiceError::Forbidden)?;

        if !member.permissions.has(permission) {
            return Err(FeedServiceError::Forbidden);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::entity_feed::{EntityFeed, EntityFeedError};
    use domain::feed::FeedError;
    use domain::feed_element::{FeedElement, FeedElementError};
    use domain::feed_item::{FeedItem, FeedItemError};
    use domain::organization_member::{
        OrganizationMember, OrganizationMemberError, OrganizationMemberRepository, Role,
    };
    use tokio::sync::Mutex;

    // --- Mock Repos -------------------------------------------------------------

    #[derive(Default)]
    struct MockFeedRepo {
        feeds: Mutex<Vec<Feed>>,
    }

    #[async_trait::async_trait]
    impl FeedRepository for MockFeedRepo {
        async fn create(
            &self,
            slug: &str,
            display_name: &str,
            description: Option<&str>,
            feed_type: FeedType,
        ) -> Result<Feed, FeedError> {
            let feed = Feed {
                id: Uuid::new_v4(),
                slug: slug.into(),
                display_name: display_name.into(),
                description: description.map(String::from),
                feed_type,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                deleted_at: None,
            };
            self.feeds.lock().await.push(feed.clone());
            Ok(feed)
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<Feed>, FeedError> {
            Ok(self.feeds.lock().await.iter().find(|f| f.id == id).cloned())
        }
        async fn update(
            &self,
            _id: Uuid,
            _display_name: &str,
            _description: Option<&str>,
        ) -> Result<Feed, FeedError> {
            unimplemented!()
        }
        async fn soft_delete(&self, id: Uuid) -> Result<(), FeedError> {
            let feeds = self.feeds.lock().await;
            let feed = feeds.iter().find(|f| f.id == id).ok_or(FeedError::NotFound)?;
            if feed.feed_type == FeedType::System {
                return Err(FeedError::SystemFeedUndeletable);
            }
            Ok(())
        }
        async fn list_by_ids(&self, ids: &[Uuid]) -> Result<Vec<Feed>, FeedError> {
            let feeds = self.feeds.lock().await;
            let result = feeds
                .iter()
                .filter(|f| ids.contains(&f.id))
                .cloned()
                .collect();
            Ok(result)
        }
    }

    #[derive(Default)]
    struct MockEntityFeedRepo {
        entity_feeds: Mutex<Vec<EntityFeed>>,
    }

    #[async_trait::async_trait]
    impl EntityFeedRepository for MockEntityFeedRepo {
        async fn attach(
            &self,
            feed_id: Uuid,
            entity_type: EntityType,
            entity_id: Uuid,
        ) -> Result<EntityFeed, EntityFeedError> {
            let ef = EntityFeed {
                feed_id,
                entity_type,
                entity_id,
            };
            self.entity_feeds.lock().await.push(ef.clone());
            Ok(ef)
        }
        async fn find_by_feed_id(
            &self,
            feed_id: Uuid,
        ) -> Result<Option<EntityFeed>, EntityFeedError> {
            Ok(self
                .entity_feeds
                .lock()
                .await
                .iter()
                .find(|ef| ef.feed_id == feed_id)
                .cloned())
        }
        async fn list_by_entity(
            &self,
            entity_type: EntityType,
            entity_id: Uuid,
        ) -> Result<Vec<EntityFeed>, EntityFeedError> {
            let efs = self.entity_feeds.lock().await;
            let result = efs
                .iter()
                .filter(|ef| ef.entity_type == entity_type && ef.entity_id == entity_id)
                .cloned()
                .collect();
            Ok(result)
        }
        async fn detach(&self, _feed_id: Uuid) -> Result<(), EntityFeedError> {
            unimplemented!()
        }
    }

    #[derive(Default)]
    struct MockFeedItemRepo {
        items: Mutex<Vec<FeedItem>>,
    }

    #[async_trait::async_trait]
    impl FeedItemRepository for MockFeedItemRepo {
        async fn create(
            &self,
            feed_id: Uuid,
            author_type: AuthorType,
            author_id: Uuid,
        ) -> Result<FeedItem, FeedItemError> {
            let item = FeedItem {
                id: Uuid::new_v4(),
                feed_id,
                author_type,
                author_id,
                created_at: chrono::Utc::now(),
            };
            self.items.lock().await.push(item.clone());
            Ok(item)
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<FeedItem>, FeedItemError> {
            Ok(self.items.lock().await.iter().find(|i| i.id == id).cloned())
        }
        async fn list_by_feed(
            &self,
            feed_id: Uuid,
            limit: i64,
            offset: i64,
        ) -> Result<Vec<FeedItem>, FeedItemError> {
            let items = self.items.lock().await;
            let result = items
                .iter()
                .filter(|i| i.feed_id == feed_id)
                .skip(offset as usize)
                .take(limit as usize)
                .cloned()
                .collect();
            Ok(result)
        }
        async fn delete(&self, id: Uuid) -> Result<(), FeedItemError> {
            let mut items = self.items.lock().await;
            let len_before = items.len();
            items.retain(|i| i.id != id);
            if items.len() == len_before {
                Err(FeedItemError::NotFound)
            } else {
                Ok(())
            }
        }
    }

    #[derive(Default)]
    struct MockFeedElementRepo {
        elements: Mutex<Vec<FeedElement>>,
    }

    #[async_trait::async_trait]
    impl FeedElementRepository for MockFeedElementRepo {
        async fn create(
            &self,
            feed_item_id: Uuid,
            element_type: FeedElementType,
            content_json: &str,
            position: i32,
        ) -> Result<FeedElement, FeedElementError> {
            let el = FeedElement {
                id: Uuid::new_v4(),
                feed_item_id,
                element_type,
                content_json: content_json.into(),
                position,
            };
            self.elements.lock().await.push(el.clone());
            Ok(el)
        }
        async fn find_by_id(&self, _id: Uuid) -> Result<Option<FeedElement>, FeedElementError> {
            Ok(None)
        }
        async fn list_by_feed_item(
            &self,
            feed_item_id: Uuid,
        ) -> Result<Vec<FeedElement>, FeedElementError> {
            let els = self.elements.lock().await;
            let result = els
                .iter()
                .filter(|e| e.feed_item_id == feed_item_id)
                .cloned()
                .collect();
            Ok(result)
        }
        async fn update_content(
            &self,
            _id: Uuid,
            _content_json: &str,
        ) -> Result<FeedElement, FeedElementError> {
            unimplemented!()
        }
        async fn delete(&self, _id: Uuid) -> Result<(), FeedElementError> {
            unimplemented!()
        }
    }

    #[derive(Default)]
    struct MockMemberRepo {
        members: Mutex<Vec<OrganizationMember>>,
    }

    #[async_trait::async_trait]
    impl OrganizationMemberRepository for MockMemberRepo {
        async fn add(
            &self,
            _org_id: Uuid,
            _user_id: Uuid,
            _role: Role,
            _title: Option<&str>,
            _permissions: Permissions,
        ) -> Result<OrganizationMember, OrganizationMemberError> {
            unimplemented!()
        }
        async fn find_by_org_and_user(
            &self,
            org_id: Uuid,
            user_id: Uuid,
        ) -> Result<Option<OrganizationMember>, OrganizationMemberError> {
            Ok(self
                .members
                .lock()
                .await
                .iter()
                .find(|m| m.org_id == org_id && m.user_id == user_id)
                .cloned())
        }
        async fn list_by_org(
            &self,
            _org_id: Uuid,
        ) -> Result<Vec<OrganizationMember>, OrganizationMemberError> {
            unimplemented!()
        }
        async fn list_by_user(
            &self,
            _user_id: Uuid,
        ) -> Result<Vec<OrganizationMember>, OrganizationMemberError> {
            unimplemented!()
        }
        async fn update_role_and_title(
            &self,
            _org_id: Uuid,
            _user_id: Uuid,
            _role: Role,
            _title: Option<&str>,
        ) -> Result<OrganizationMember, OrganizationMemberError> {
            unimplemented!()
        }
        async fn update_permissions(
            &self,
            _org_id: Uuid,
            _user_id: Uuid,
            _permissions: Permissions,
        ) -> Result<OrganizationMember, OrganizationMemberError> {
            unimplemented!()
        }
        async fn remove(
            &self,
            _org_id: Uuid,
            _user_id: Uuid,
        ) -> Result<(), OrganizationMemberError> {
            unimplemented!()
        }
    }

    // --- Helpers ----------------------------------------------------------------

    fn test_member(org_id: Uuid, user_id: Uuid) -> OrganizationMember {
        OrganizationMember {
            id: Uuid::new_v4(),
            org_id,
            user_id,
            role: Role::Owner,
            title: None,
            permissions: Permissions::new(Permissions::ALL),
            joined_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn build_service(
        feed_repo: MockFeedRepo,
        entity_feed_repo: MockEntityFeedRepo,
        feed_item_repo: MockFeedItemRepo,
        feed_element_repo: MockFeedElementRepo,
        member_repo: MockMemberRepo,
    ) -> FeedService {
        FeedService::new(
            Arc::new(feed_repo),
            Arc::new(entity_feed_repo),
            Arc::new(feed_item_repo),
            Arc::new(feed_element_repo),
            Arc::new(member_repo),
        )
    }

    // --- Tests ------------------------------------------------------------------

    #[tokio::test]
    async fn list_feeds_for_entity_returns_attached_feeds() {
        let org_id = Uuid::new_v4();
        let feed_repo = MockFeedRepo::default();
        let entity_feed_repo = MockEntityFeedRepo::default();

        // Create a feed and attach it
        let feed = feed_repo
            .create("updates", "Updates", None, FeedType::System)
            .await
            .unwrap();
        entity_feed_repo
            .attach(feed.id, EntityType::Org, org_id)
            .await
            .unwrap();

        let svc = build_service(
            feed_repo,
            entity_feed_repo,
            MockFeedItemRepo::default(),
            MockFeedElementRepo::default(),
            MockMemberRepo::default(),
        );

        let feeds = svc
            .list_feeds_for_entity(EntityType::Org, org_id)
            .await
            .unwrap();

        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].slug, "updates");
    }

    #[tokio::test]
    async fn create_custom_feed_attaches_to_org() {
        let org_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let svc = build_service(
            MockFeedRepo::default(),
            MockEntityFeedRepo::default(),
            MockFeedItemRepo::default(),
            MockFeedElementRepo::default(),
            MockMemberRepo {
                members: Mutex::new(vec![test_member(org_id, user_id)]),
            },
        );

        let feed = svc
            .create_custom_feed(org_id, user_id, "my-feed", "My Feed", Some("A custom feed"))
            .await
            .unwrap();

        assert_eq!(feed.slug, "my-feed");
        assert_eq!(feed.feed_type, FeedType::Custom);
    }

    #[tokio::test]
    async fn create_custom_feed_without_permission_fails() {
        let org_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Member with no permissions
        let member = OrganizationMember {
            id: Uuid::new_v4(),
            org_id,
            user_id,
            role: Role::Member,
            title: None,
            permissions: Permissions::default(),
            joined_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let svc = build_service(
            MockFeedRepo::default(),
            MockEntityFeedRepo::default(),
            MockFeedItemRepo::default(),
            MockFeedElementRepo::default(),
            MockMemberRepo {
                members: Mutex::new(vec![member]),
            },
        );

        let err = svc
            .create_custom_feed(org_id, user_id, "my-feed", "My Feed", None)
            .await
            .unwrap_err();

        assert!(matches!(err, FeedServiceError::Forbidden));
    }

    #[tokio::test]
    async fn delete_system_feed_fails() {
        let org_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let feed_repo = MockFeedRepo::default();
        let entity_feed_repo = MockEntityFeedRepo::default();

        let feed = feed_repo
            .create("updates", "Updates", None, FeedType::System)
            .await
            .unwrap();
        entity_feed_repo
            .attach(feed.id, EntityType::Org, org_id)
            .await
            .unwrap();

        let svc = build_service(
            feed_repo,
            entity_feed_repo,
            MockFeedItemRepo::default(),
            MockFeedElementRepo::default(),
            MockMemberRepo {
                members: Mutex::new(vec![test_member(org_id, user_id)]),
            },
        );

        let err = svc.delete_feed(feed.id, user_id).await.unwrap_err();
        assert!(matches!(err, FeedServiceError::SystemFeedUndeletable));
    }

    #[tokio::test]
    async fn post_to_feed_creates_item_and_elements() {
        let org_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let feed_repo = MockFeedRepo::default();
        let entity_feed_repo = MockEntityFeedRepo::default();

        let feed = feed_repo
            .create("updates", "Updates", None, FeedType::System)
            .await
            .unwrap();
        entity_feed_repo
            .attach(feed.id, EntityType::Org, org_id)
            .await
            .unwrap();

        let svc = build_service(
            feed_repo,
            entity_feed_repo,
            MockFeedItemRepo::default(),
            MockFeedElementRepo::default(),
            MockMemberRepo {
                members: Mutex::new(vec![test_member(org_id, user_id)]),
            },
        );

        let result = svc
            .post_to_feed(
                feed.id,
                user_id,
                vec![
                    NewFeedElement {
                        element_type: FeedElementType::Text,
                        content_json: r#"{"text":"Hello!"}"#.into(),
                        position: 0,
                    },
                    NewFeedElement {
                        element_type: FeedElementType::Image,
                        content_json: r#"{"url":"https://example.com/img.png"}"#.into(),
                        position: 1,
                    },
                ],
            )
            .await
            .unwrap();

        assert_eq!(result.item.feed_id, feed.id);
        assert_eq!(result.elements.len(), 2);
    }

    #[tokio::test]
    async fn list_feed_items_returns_items_with_elements() {
        let feed_item_repo = MockFeedItemRepo::default();
        let feed_element_repo = MockFeedElementRepo::default();
        let feed_id = Uuid::new_v4();
        let author_id = Uuid::new_v4();

        let item = feed_item_repo
            .create(feed_id, AuthorType::User, author_id)
            .await
            .unwrap();
        feed_element_repo
            .create(item.id, FeedElementType::Text, r#"{"text":"hi"}"#, 0)
            .await
            .unwrap();

        let svc = build_service(
            MockFeedRepo::default(),
            MockEntityFeedRepo::default(),
            feed_item_repo,
            feed_element_repo,
            MockMemberRepo::default(),
        );

        let items = svc.list_feed_items(feed_id, 10, 0).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].elements.len(), 1);
    }
}
