use std::sync::Arc;

use domain::entity_feed::{EntityFeedRepository, EntityType};
use domain::feed::{Feed, FeedRepository, FeedType};
use domain::onboarding_role::OnboardingRole;
use domain::organization::OrganizationRepository;
use domain::user::UserRepository;
use uuid::Uuid;

/// Errors from onboarding operations.
#[derive(Debug, thiserror::Error)]
pub enum OnboardingError {
    #[error("User not found")]
    UserNotFound,
    #[error("Personal organization not found for user")]
    PersonalOrgNotFound,
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result of completing onboarding: feeds created and role selected.
#[derive(Debug)]
pub struct OnboardingResult {
    pub feeds_created: Vec<Feed>,
    pub onboarding_role: OnboardingRole,
}

/// Orchestrates first-login onboarding: creates system feeds based on selected role.
pub struct OnboardingService {
    user_repo: Arc<dyn UserRepository>,
    org_repo: Arc<dyn OrganizationRepository>,
    feed_repo: Arc<dyn FeedRepository>,
    entity_feed_repo: Arc<dyn EntityFeedRepository>,
}

impl OnboardingService {
    /// Create a new onboarding service with all required repositories.
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        org_repo: Arc<dyn OrganizationRepository>,
        feed_repo: Arc<dyn FeedRepository>,
        entity_feed_repo: Arc<dyn EntityFeedRepository>,
    ) -> Self {
        Self {
            user_repo,
            org_repo,
            feed_repo,
            entity_feed_repo,
        }
    }

    /// Complete onboarding for a user: create system feeds on their personal org and mark done.
    pub async fn complete_onboarding(
        &self,
        user_id: Uuid,
        role: OnboardingRole,
    ) -> Result<OnboardingResult, OnboardingError> {
        // 1. Find user
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await
            .map_err(|e| OnboardingError::Internal(e.to_string()))?
            .ok_or(OnboardingError::UserNotFound)?;

        // 2. Idempotent: if already completed, return early
        if user.onboarding_completed_at.is_some() {
            return Ok(OnboardingResult {
                feeds_created: vec![],
                onboarding_role: role,
            });
        }

        // 3. Find personal org
        let org = self
            .org_repo
            .find_personal_org(user_id)
            .await
            .map_err(|e| OnboardingError::Internal(e.to_string()))?
            .ok_or(OnboardingError::PersonalOrgNotFound)?;

        // 4. Check existing feeds to avoid duplicates
        let existing_entity_feeds = self
            .entity_feed_repo
            .list_by_entity(EntityType::Org, org.id)
            .await
            .map_err(|e| OnboardingError::Internal(e.to_string()))?;

        let existing_slugs: Vec<String> = {
            let feed_ids: Vec<Uuid> = existing_entity_feeds.iter().map(|ef| ef.feed_id).collect();
            if feed_ids.is_empty() {
                vec![]
            } else {
                let feeds = self
                    .feed_repo
                    .list_by_ids(&feed_ids)
                    .await
                    .map_err(|e| OnboardingError::Internal(e.to_string()))?;
                feeds.into_iter().map(|f| f.slug).collect()
            }
        };

        // TODO(review): feed slugs are hardcoded; should be a shared constant or config if these evolve
        // 5. Create missing system feeds
        let mut desired_feeds = vec![
            ("bio", "Bio"),
            ("updates", "Updates"),
            ("gallery", "Gallery"),
        ];
        if role.creates_commissions_feed() {
            desired_feeds.push(("commissions", "Commissions"));
        }

        let mut feeds_created = Vec::new();
        for (slug, display_name) in desired_feeds {
            if existing_slugs.contains(&slug.to_string()) {
                continue;
            }

            let feed = self
                .feed_repo
                .create_and_attach(slug, display_name, None, FeedType::System, EntityType::Org, org.id)
                .await
                .map_err(|e| OnboardingError::Internal(e.to_string()))?;

            feeds_created.push(feed);
        }

        // TODO(Feature 3.5 Phase 2): the full loop (all feeds + mark onboarding complete) is still not atomic — needs UoW
        // 6. Mark onboarding completed
        self.user_repo
            .mark_onboarding_completed(user_id)
            .await
            .map_err(|e| OnboardingError::Internal(e.to_string()))?;

        Ok(OnboardingResult {
            feeds_created,
            onboarding_role: role,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::entity_feed::{EntityFeed, EntityFeedError, EntityFeedRepository, EntityType};
    use domain::feed::{Feed, FeedError, FeedRepository, FeedType};
    use domain::organization::{Organization, OrganizationError, OrganizationRepository};
    use domain::user::{User, UserError, UserRepository};
    use tokio::sync::Mutex;

    // --- Mock Repos -------------------------------------------------------------

    #[derive(Default)]
    struct MockUserRepo {
        users: Mutex<Vec<User>>,
    }

    #[async_trait::async_trait]
    impl UserRepository for MockUserRepo {
        async fn find_by_email(&self, _email: &str) -> Result<Option<User>, UserError> {
            Ok(None)
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, UserError> {
            Ok(self.users.lock().await.iter().find(|u| u.id == id).cloned())
        }
        async fn find_by_did(&self, _did: &str) -> Result<Option<User>, UserError> {
            Ok(None)
        }
        async fn create_from_atproto(
            &self,
            _did: &str,
            _handle: Option<&str>,
            _email: Option<&str>,
        ) -> Result<User, UserError> {
            unimplemented!()
        }
        async fn update_handle(&self, _user_id: Uuid, _handle: &str) -> Result<(), UserError> {
            Ok(())
        }
        async fn mark_onboarding_completed(&self, user_id: Uuid) -> Result<(), UserError> {
            let mut users = self.users.lock().await;
            let user = users
                .iter_mut()
                .find(|u| u.id == user_id)
                .ok_or(UserError::NotFound)?;
            user.onboarding_completed_at = Some(chrono::Utc::now());
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockOrgRepo {
        orgs: Mutex<Vec<Organization>>,
    }

    #[async_trait::async_trait]
    impl OrganizationRepository for MockOrgRepo {
        async fn create(
            &self,
            _slug: &str,
            _display_name: Option<&str>,
            _is_personal: bool,
        ) -> Result<Organization, OrganizationError> {
            unimplemented!()
        }
        async fn find_by_id(&self, _id: Uuid) -> Result<Option<Organization>, OrganizationError> {
            Ok(None)
        }
        async fn find_by_slug(
            &self,
            _slug: &str,
        ) -> Result<Option<Organization>, OrganizationError> {
            Ok(None)
        }
        async fn find_personal_org(
            &self,
            user_id: Uuid,
        ) -> Result<Option<Organization>, OrganizationError> {
            Ok(self
                .orgs
                .lock()
                .await
                .iter()
                .find(|o| o.is_personal)
                .cloned())
        }
        async fn update_display_name(
            &self,
            _id: Uuid,
            _display_name: Option<&str>,
        ) -> Result<Organization, OrganizationError> {
            unimplemented!()
        }
        async fn soft_delete(&self, _id: Uuid) -> Result<(), OrganizationError> {
            unimplemented!()
        }
    }

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
        async fn find_by_id(&self, _id: Uuid) -> Result<Option<Feed>, FeedError> {
            Ok(None)
        }
        async fn update(
            &self,
            _id: Uuid,
            _display_name: &str,
            _description: Option<&str>,
        ) -> Result<Feed, FeedError> {
            unimplemented!()
        }
        async fn soft_delete(&self, _id: Uuid) -> Result<(), FeedError> {
            unimplemented!()
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
        async fn create_and_attach(
            &self,
            slug: &str,
            display_name: &str,
            description: Option<&str>,
            feed_type: FeedType,
            _entity_type: domain::entity_feed::EntityType,
            _entity_id: Uuid,
        ) -> Result<Feed, FeedError> {
            self.create(slug, display_name, description, feed_type).await
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
            _feed_id: Uuid,
        ) -> Result<Option<EntityFeed>, EntityFeedError> {
            Ok(None)
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

    // --- Helpers ----------------------------------------------------------------

    fn test_user(id: Uuid) -> User {
        User {
            id,
            did: Some("did:plc:test".into()),
            handle: Some("test.bsky.social".into()),
            email: None,
            username: "test".into(),
            onboarding_completed_at: None,
        }
    }

    fn test_org(id: Uuid, user_id: Uuid) -> Organization {
        Organization {
            id,
            slug: "test".into(),
            display_name: None,
            is_personal: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn build_service(
        user_repo: MockUserRepo,
        org_repo: MockOrgRepo,
        feed_repo: MockFeedRepo,
        entity_feed_repo: MockEntityFeedRepo,
    ) -> OnboardingService {
        OnboardingService::new(
            Arc::new(user_repo),
            Arc::new(org_repo),
            Arc::new(feed_repo),
            Arc::new(entity_feed_repo),
        )
    }

    // --- Tests ------------------------------------------------------------------

    #[tokio::test]
    async fn complete_onboarding_for_artist_creates_three_feeds() {
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        let svc = build_service(
            MockUserRepo {
                users: Mutex::new(vec![test_user(user_id)]),
            },
            MockOrgRepo {
                orgs: Mutex::new(vec![test_org(org_id, user_id)]),
            },
            MockFeedRepo::default(),
            MockEntityFeedRepo::default(),
        );

        let result = svc
            .complete_onboarding(user_id, OnboardingRole::Artist)
            .await
            .unwrap();

        assert_eq!(result.feeds_created.len(), 4);
        let slugs: Vec<&str> = result.feeds_created.iter().map(|f| f.slug.as_str()).collect();
        assert!(slugs.contains(&"bio"));
        assert!(slugs.contains(&"updates"));
        assert!(slugs.contains(&"gallery"));
        assert!(slugs.contains(&"commissions"));
    }

    #[tokio::test]
    async fn complete_onboarding_for_commissioner_creates_two_feeds() {
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        let svc = build_service(
            MockUserRepo {
                users: Mutex::new(vec![test_user(user_id)]),
            },
            MockOrgRepo {
                orgs: Mutex::new(vec![test_org(org_id, user_id)]),
            },
            MockFeedRepo::default(),
            MockEntityFeedRepo::default(),
        );

        let result = svc
            .complete_onboarding(user_id, OnboardingRole::CommissionerClient)
            .await
            .unwrap();

        assert_eq!(result.feeds_created.len(), 3);
        let slugs: Vec<&str> = result.feeds_created.iter().map(|f| f.slug.as_str()).collect();
        assert!(slugs.contains(&"bio"));
        assert!(slugs.contains(&"updates"));
        assert!(slugs.contains(&"gallery"));
        assert!(!slugs.contains(&"commissions"));
    }

    #[tokio::test]
    async fn complete_onboarding_is_idempotent() {
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        let mut user = test_user(user_id);
        user.onboarding_completed_at = Some(chrono::Utc::now());

        let svc = build_service(
            MockUserRepo {
                users: Mutex::new(vec![user]),
            },
            MockOrgRepo {
                orgs: Mutex::new(vec![test_org(org_id, user_id)]),
            },
            MockFeedRepo::default(),
            MockEntityFeedRepo::default(),
        );

        let result = svc
            .complete_onboarding(user_id, OnboardingRole::Artist)
            .await
            .unwrap();

        assert!(result.feeds_created.is_empty());
    }

    #[tokio::test]
    async fn complete_onboarding_for_nonexistent_user_fails() {
        let svc = build_service(
            MockUserRepo::default(),
            MockOrgRepo::default(),
            MockFeedRepo::default(),
            MockEntityFeedRepo::default(),
        );

        let err = svc
            .complete_onboarding(Uuid::new_v4(), OnboardingRole::Artist)
            .await
            .unwrap_err();

        assert!(matches!(err, OnboardingError::UserNotFound));
    }

    #[tokio::test]
    async fn complete_onboarding_without_personal_org_fails() {
        let user_id = Uuid::new_v4();

        let svc = build_service(
            MockUserRepo {
                users: Mutex::new(vec![test_user(user_id)]),
            },
            MockOrgRepo::default(),
            MockFeedRepo::default(),
            MockEntityFeedRepo::default(),
        );

        let err = svc
            .complete_onboarding(user_id, OnboardingRole::Artist)
            .await
            .unwrap_err();

        assert!(matches!(err, OnboardingError::PersonalOrgNotFound));
    }
}
