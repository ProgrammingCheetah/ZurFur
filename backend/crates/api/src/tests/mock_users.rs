//! Mock implementations for user-related repository traits.

use async_trait::async_trait;
use domain::content_rating::ContentRating;
use domain::user::{User, UserError, UserRepository};
use domain::user_preferences::{UserPreferences, UserPreferencesError, UserPreferencesRepository};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub struct MockUserRepo {
    pub users: Mutex<Vec<User>>,
}

#[async_trait]
impl UserRepository for MockUserRepo {
    async fn find_by_email(&self, _email: &str) -> Result<Option<User>, UserError> {
        Ok(None)
    }
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, UserError> {
        let users = self.users.lock().await;
        let user = users.iter().find(|u| u.id == id).cloned();
        Ok(user)
    }
    async fn find_by_did(&self, did: &str) -> Result<Option<User>, UserError> {
        let users = self.users.lock().await;
        let user = users.iter().find(|u| u.did.as_deref() == Some(did)).cloned();
        Ok(user)
    }
    async fn create_from_atproto(
        &self,
        did: &str,
        handle: Option<&str>,
        email: Option<&str>,
    ) -> Result<User, UserError> {
        let user = User {
            id: Uuid::new_v4(),
            did: Some(did.to_string()),
            handle: handle.map(String::from),
            email: email.map(String::from),
            username: handle.unwrap_or(did).to_string(),
            onboarding_completed_at: None,
        };
        self.users.lock().await.push(user.clone());
        Ok(user)
    }
    async fn update_handle(&self, _user_id: Uuid, _handle: &str) -> Result<(), UserError> {
        Ok(())
    }
    async fn mark_onboarding_completed(&self, _user_id: Uuid) -> Result<(), UserError> {
        Ok(())
    }
}

#[derive(Default)]
pub struct MockPreferencesRepo {
    pub prefs: Mutex<Vec<UserPreferences>>,
}

#[async_trait]
impl UserPreferencesRepository for MockPreferencesRepo {
    async fn get(&self, user_id: Uuid) -> Result<UserPreferences, UserPreferencesError> {
        let prefs = self.prefs.lock().await;
        let pref = prefs
            .iter()
            .find(|p| p.user_id == user_id)
            .cloned()
            .unwrap_or(UserPreferences {
                user_id,
                max_content_rating: ContentRating::Sfw,
            });
        Ok(pref)
    }
    async fn set_max_content_rating(
        &self,
        user_id: Uuid,
        rating: ContentRating,
    ) -> Result<UserPreferences, UserPreferencesError> {
        let mut prefs = self.prefs.lock().await;
        prefs.retain(|p| p.user_id != user_id);
        let updated = UserPreferences {
            user_id,
            max_content_rating: rating,
        };
        prefs.push(updated.clone());
        Ok(updated)
    }
}
