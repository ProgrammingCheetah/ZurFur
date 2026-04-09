//! AuthService orchestrator: wires AT Protocol OAuth with user persistence and JWT sessions.

use std::num::NonZeroUsize;
use std::sync::Arc;

use chrono::{Duration, Utc};
use domain::atproto_session::{AtprotoSessionEntity, AtprotoSessionRepository};
use domain::oauth_state_store::{OAuthStateData, OAuthStateStore};
use domain::organization::OrganizationRepository;
use domain::organization_member::OrganizationMemberRepository;
use domain::refresh_token::RefreshTokenRepository;
use domain::user::UserRepository;
use shared::JwtConfig;
use uuid::Uuid;

use crate::organization::service::OrganizationService;

/// No-op implementation of OrganizationProfileRepository used when only
/// org + member repos are needed (e.g., personal org creation in auth flow).
struct NoOpProfileRepo;

#[async_trait::async_trait]
impl domain::organization_profile::OrganizationProfileRepository for NoOpProfileRepo {
    async fn upsert(
        &self,
        _org_id: Uuid,
        _bio: Option<&str>,
        _status: domain::organization_profile::CommissionStatus,
    ) -> Result<domain::organization_profile::OrganizationProfile, domain::organization_profile::OrganizationProfileError> {
        unimplemented!("NoOpProfileRepo::upsert should not be called during personal org creation")
    }
    async fn find_by_org_id(
        &self,
        _org_id: Uuid,
    ) -> Result<Option<domain::organization_profile::OrganizationProfile>, domain::organization_profile::OrganizationProfileError> {
        Ok(None)
    }
}

use super::login::{
    LoginError, OAuthConfig, complete_oauth_login, default_oauth_storage,
    resolve_identity_document, start_oauth_login,
};

use atproto_oauth::storage::OAuthRequestStorage;

/// Claims embedded in Zurfur platform JWTs (short-lived access tokens).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ZurfurClaims {
    /// User UUID
    pub sub: String,
    /// AT Protocol DID
    pub did: String,
    pub handle: Option<String>,
    /// Unix timestamp expiration
    pub exp: i64,
}

/// Returned by `POST /auth/start`.
pub struct StartLoginResponse {
    pub redirect_url: String,
    pub state: String,
}

/// Returned by `GET /auth/callback`.
pub struct CompleteLoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: Uuid,
    pub did: String,
    pub handle: Option<String>,
    pub is_new_user: bool,
}

/// Returned by `POST /auth/refresh`.
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
}

/// The central authentication service. Holds all dependencies and orchestrates the OAuth flow,
/// user creation, session storage, and JWT issuance.
pub struct AuthService<S: OAuthRequestStorage> {
    oauth_config: OAuthConfig,
    jwt_config: JwtConfig,
    oauth_storage: Arc<S>,
    state_store: Arc<dyn OAuthStateStore>,
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn AtprotoSessionRepository>,
    refresh_repo: Arc<dyn RefreshTokenRepository>,
    org_repo: Arc<dyn OrganizationRepository>,
    member_repo: Arc<dyn OrganizationMemberRepository>,
}

impl<S: OAuthRequestStorage> AuthService<S> {
    pub fn new(
        oauth_config: OAuthConfig,
        jwt_config: JwtConfig,
        oauth_storage: Arc<S>,
        state_store: Arc<dyn OAuthStateStore>,
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn AtprotoSessionRepository>,
        refresh_repo: Arc<dyn RefreshTokenRepository>,
        org_repo: Arc<dyn OrganizationRepository>,
        member_repo: Arc<dyn OrganizationMemberRepository>,
    ) -> Self {
        Self {
            oauth_config,
            jwt_config,
            oauth_storage,
            state_store,
            user_repo,
            session_repo,
            refresh_repo,
            org_repo,
            member_repo,
        }
    }

    /// Derive the public JWK from the private signing key. Used by the
    /// /client-metadata.json endpoint so Bluesky can verify our client assertions.
    ///
    /// Uses `elliptic_curve::JwkEcKey` for correct JWK serialization — this matches
    /// what the atproto-oauth crate uses internally for JWT header construction.
    pub fn public_jwk(&self) -> Result<serde_json::Value, String> {
        use atproto_identity::key::to_public;

        let public_key = to_public(&self.oauth_config.private_signing_key_data)
            .map_err(|e| format!("Failed to derive public key: {e}"))?;
        // The atproto-oauth crate uses `public_key.to_string()` (did:key:...) as the JWT `kid`
        let kid = public_key.to_string();
        let jwk: elliptic_curve::JwkEcKey = (&public_key)
            .try_into()
            .map_err(|e| format!("Failed to convert public key to JWK: {e}"))?;
        let mut jwk_value = serde_json::to_value(&jwk)
            .map_err(|e| format!("Failed to serialize JWK: {e}"))?;
        if let Some(obj) = jwk_value.as_object_mut() {
            obj.insert("use".into(), serde_json::json!("sig"));
            obj.insert("kid".into(), serde_json::json!(kid));
            obj.insert("alg".into(), serde_json::json!("ES256"));
        }
        Ok(jwk_value)
    }

    /// Get the OAuth client_id (URL to client-metadata.json).
    pub fn client_id(&self) -> &str {
        &self.oauth_config.client_id
    }

    /// Get the OAuth redirect_uri.
    pub fn redirect_uri(&self) -> &str {
        &self.oauth_config.redirect_uri
    }

    /// Step 1: Start the OAuth login flow. Returns a redirect URL for the user's browser.
    pub async fn start_login(
        &self,
        handle_or_did: &str,
    ) -> Result<StartLoginResponse, LoginError> {
        // Resolve identity once — the document is reused for OAuth init
        let document = resolve_identity_document(handle_or_did, &self.oauth_config.plc_hostname).await?;
        let did = &document.id;

        let result = start_oauth_login(
            handle_or_did,
            &document,
            &self.oauth_config,
            self.oauth_storage.as_ref(),
        )
        .await?;

        // Store resolved identity keyed by state so the callback can look it up securely.
        let handle = if handle_or_did.starts_with("did:") {
            None
        } else {
            Some(handle_or_did.to_string())
        };
        self.state_store
            .store(
                &result.state,
                OAuthStateData {
                    did: did.to_string(),
                    handle,
                },
            )
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?;

        Ok(StartLoginResponse {
            redirect_url: result.redirect_url,
            state: result.state,
        })
    }

    /// Step 2: Complete the OAuth flow after Bluesky redirects back with code + state.
    /// Finds or creates the user, stores AT Protocol tokens, issues JWT + refresh token.
    pub async fn complete_login(
        &self,
        code: &str,
        state: &str,
    ) -> Result<CompleteLoginResponse, LoginError> {
        // Retrieve DID and handle from server-side storage (prevents client-side tampering)
        let state_data = self
            .state_store
            .take(state)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?
            .ok_or(LoginError::InvalidState)?;

        let did = state_data.did;
        let handle = state_data.handle;

        // Exchange code for AT Protocol tokens
        let session = complete_oauth_login(
            code,
            state,
            &did,
            handle.as_deref(),
            &self.oauth_config,
            self.oauth_storage.as_ref(),
        )
        .await?;

        // Find or create user
        let (user, is_new) = match self
            .user_repo
            .find_by_did(&session.did)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?
        {
            Some(mut user) => {
                // Update handle if it changed (handles can be reassigned on Bluesky)
                if let Some(new_handle) = &session.handle {
                    if user.handle.as_deref() != Some(new_handle) {
                        self.user_repo
                            .update_handle(user.id, new_handle)
                            .await
                            .map_err(|e| LoginError::InternalError(e.to_string()))?;
                        user.handle = Some(new_handle.clone());
                    }
                }

                // Self-healing: ensure personal org exists for returning users
                // (covers users created before the org model was introduced).
                let has_personal_org = self
                    .org_repo
                    .find_personal_org(user.id)
                    .await
                    .map_err(|e| LoginError::InternalError(e.to_string()))?
                    .is_some();

                if !has_personal_org {
                    let slug = user
                        .handle
                        .as_deref()
                        .map(OrganizationService::slug_from_handle)
                        .unwrap_or_else(|| {
                            OrganizationService::slug_from_handle(
                                user.did.as_deref().unwrap_or("user"),
                            )
                        });
                    let org_service = OrganizationService::new(
                        self.org_repo.clone(),
                        self.member_repo.clone(),
                        Arc::new(NoOpProfileRepo),
                    );
                    if let Err(e) = org_service.create_personal_org(user.id, &slug).await {
                        eprintln!(
                            "Failed to create personal org for returning user {}: {e}",
                            user.id
                        );
                    }
                }

                (user, false)
            }
            None => {
                let user = self
                    .user_repo
                    .create_from_atproto(
                        &session.did,
                        session.handle.as_deref(),
                        session.email.as_deref(),
                    )
                    .await
                    .map_err(|e| LoginError::InternalError(e.to_string()))?;

                // Create personal org for the new user.
                //
                // ARCHITECTURE DECISIONS:
                //   Personal org auto-creation happens here during signup, not as
                //   a separate onboarding step. The personal org IS the user's
                //   public profile — bio, title, display name all live here.
                //   display_name is NULL (resolved from owner's handle at API layer).
                let slug = session
                    .handle
                    .as_deref()
                    .map(OrganizationService::slug_from_handle)
                    .unwrap_or_else(|| {
                        OrganizationService::slug_from_handle(&session.did)
                    });

                let org_service = OrganizationService::new(
                    self.org_repo.clone(),
                    self.member_repo.clone(),
                    // Profile repo not needed for personal org creation — pass a
                    // no-op. OrganizationService::create_personal_org only touches
                    // org_repo and member_repo.
                    Arc::new(NoOpProfileRepo),
                );
                if let Err(e) = org_service.create_personal_org(user.id, &slug).await {
                    eprintln!(
                        "Failed to create personal org for user {}: {e}",
                        user.id
                    );
                    // Non-fatal: user exists but personal org creation failed.
                    // The user can still authenticate; the org will be created
                    // on next login or via a self-healing check. TODO: retry logic.
                }

                (user, true)
            }
        };

        // Store AT Protocol tokens for later Bluesky API calls (sub-features 1.2-1.4)
        let atproto_session = AtprotoSessionEntity {
            id: Uuid::new_v4(),
            user_id: user.id,
            did: session.did.clone(),
            access_token: session.access_token,
            refresh_token: session.refresh_token,
            // Clamp expiry to reasonable bounds (1 second to 1 year)
            expires_at: Utc::now()
                + Duration::seconds((session.expires_in_secs as i64).clamp(1, 365 * 24 * 3600)),
            pds_url: None,
        };
        self.session_repo
            .upsert(&atproto_session)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?;

        // Issue Zurfur access JWT
        let access_token = self.issue_access_jwt(&user.id, &session.did, &user.handle)?;

        // Generate and store refresh token
        let refresh_token = self.create_refresh_token(user.id).await?;

        Ok(CompleteLoginResponse {
            access_token,
            refresh_token,
            user_id: user.id,
            did: session.did,
            handle: user.handle,
            is_new_user: is_new,
        })
    }

    /// Rotate a refresh token: atomically consume the old one, issue a new pair.
    pub async fn refresh_session(
        &self,
        raw_refresh_token: &str,
    ) -> Result<RefreshResponse, LoginError> {
        let token_hash = hash_token(raw_refresh_token);

        // Atomic take: DELETE ... RETURNING prevents TOCTOU race.
        // If two concurrent requests race, only one gets the row back.
        let stored = self
            .refresh_repo
            .take_by_hash(&token_hash)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?
            .ok_or(LoginError::InvalidState)?;

        // Check expiration (token was already deleted, so no cleanup needed)
        if stored.expires_at < Utc::now() {
            return Err(LoginError::InvalidState);
        }

        // Look up user to populate JWT claims
        let user = self
            .user_repo
            .find_by_id(stored.user_id)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?
            .ok_or(LoginError::UserNotFound)?;

        let did = user
            .did
            .as_deref()
            .ok_or_else(|| LoginError::InternalError("User has no DID".into()))?;

        let access_token = self.issue_access_jwt(&user.id, did, &user.handle)?;
        let refresh_token = self.create_refresh_token(user.id).await?;

        Ok(RefreshResponse {
            access_token,
            refresh_token,
        })
    }

    /// Logout: delete all refresh tokens and AT Protocol session for a user.
    pub async fn logout(&self, user_id: Uuid) -> Result<(), LoginError> {
        self.refresh_repo
            .delete_all_for_user(user_id)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?;

        self.session_repo
            .delete_by_user_id(user_id)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?;

        Ok(())
    }

    /// Verify an access token and return the decoded claims.
    /// Use this instead of accessing jwt_config directly.
    pub fn verify_access_token(&self, token: &str) -> Result<ZurfurClaims, LoginError> {
        shared::jwt::verify(token, &self.jwt_config.secret)
            .map_err(|e| LoginError::InternalError(e.to_string()))
    }

    // --- Private helpers --------------------------------------------------------

    fn issue_access_jwt(
        &self,
        user_id: &Uuid,
        did: &str,
        handle: &Option<String>,
    ) -> Result<String, LoginError> {
        let claims = ZurfurClaims {
            sub: user_id.to_string(),
            did: did.to_string(),
            handle: handle.clone(),
            exp: (Utc::now() + Duration::seconds(self.jwt_config.access_expiry_secs as i64))
                .timestamp(),
        };
        shared::jwt::create(&claims, &self.jwt_config.secret)
            .map_err(|e| LoginError::InternalError(e.to_string()))
    }

    async fn create_refresh_token(&self, user_id: Uuid) -> Result<String, LoginError> {
        let raw_token = Uuid::new_v4().to_string();
        let token_hash = hash_token(&raw_token);
        let expires_at =
            Utc::now() + Duration::seconds(self.jwt_config.refresh_expiry_secs as i64);

        self.refresh_repo
            .create(user_id, &token_hash, expires_at)
            .await
            .map_err(|e| LoginError::InternalError(e.to_string()))?;

        Ok(raw_token)
    }
}

/// Default constructor for in-memory OAuth request storage (LRU).
pub fn create_default_oauth_storage(
    capacity: NonZeroUsize,
) -> Arc<atproto_oauth::storage_lru::LruOAuthRequestStorage> {
    Arc::new(default_oauth_storage(capacity))
}

/// SHA-256 hash a raw refresh token for storage.
fn hash_token(raw: &str) -> String {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(raw.as_bytes());
    format!("{:x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use domain::atproto_session::{AtprotoSessionEntity, AtprotoSessionRepository};
    use domain::oauth_state_store::{OAuthStateData, OAuthStateError, OAuthStateStore};
    use domain::organization::{Organization, OrganizationError, OrganizationRepository};
    use domain::organization_member::{
        OrganizationMember, OrganizationMemberError, OrganizationMemberRepository, Permissions,
    };
    use domain::refresh_token::{RefreshTokenEntity, RefreshTokenRepository};
    use domain::user::{User, UserError, UserRepository};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // --- Mock repositories -------------------------------------------------------

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
        async fn find_by_did(&self, did: &str) -> Result<Option<User>, UserError> {
            Ok(self
                .users
                .lock()
                .await
                .iter()
                .find(|u| u.did.as_deref() == Some(did))
                .cloned())
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
            };
            self.users.lock().await.push(user.clone());
            Ok(user)
        }
        async fn update_handle(&self, _user_id: Uuid, _handle: &str) -> Result<(), UserError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockSessionRepo {
        sessions: Mutex<Vec<AtprotoSessionEntity>>,
    }

    #[async_trait::async_trait]
    impl AtprotoSessionRepository for MockSessionRepo {
        async fn upsert(&self, session: &AtprotoSessionEntity) -> Result<(), UserError> {
            let mut sessions = self.sessions.lock().await;
            sessions.retain(|s| s.user_id != session.user_id);
            sessions.push(session.clone());
            Ok(())
        }
        async fn find_by_user_id(
            &self,
            user_id: Uuid,
        ) -> Result<Option<AtprotoSessionEntity>, UserError> {
            Ok(self
                .sessions
                .lock()
                .await
                .iter()
                .find(|s| s.user_id == user_id)
                .cloned())
        }
        async fn find_by_did(
            &self,
            did: &str,
        ) -> Result<Option<AtprotoSessionEntity>, UserError> {
            Ok(self
                .sessions
                .lock()
                .await
                .iter()
                .find(|s| s.did == did)
                .cloned())
        }
        async fn delete_by_user_id(&self, user_id: Uuid) -> Result<(), UserError> {
            self.sessions.lock().await.retain(|s| s.user_id != user_id);
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockRefreshRepo {
        tokens: Mutex<Vec<RefreshTokenEntity>>,
    }

    #[async_trait::async_trait]
    impl RefreshTokenRepository for MockRefreshRepo {
        async fn create(
            &self,
            user_id: Uuid,
            token_hash: &str,
            expires_at: DateTime<Utc>,
        ) -> Result<(), UserError> {
            self.tokens.lock().await.push(RefreshTokenEntity {
                id: Uuid::new_v4(),
                user_id,
                token_hash: token_hash.to_string(),
                expires_at,
                created_at: Utc::now(),
            });
            Ok(())
        }
        async fn find_by_hash(
            &self,
            token_hash: &str,
        ) -> Result<Option<RefreshTokenEntity>, UserError> {
            Ok(self
                .tokens
                .lock()
                .await
                .iter()
                .find(|t| t.token_hash == token_hash)
                .cloned())
        }
        async fn take_by_hash(
            &self,
            token_hash: &str,
        ) -> Result<Option<RefreshTokenEntity>, UserError> {
            let mut tokens = self.tokens.lock().await;
            if let Some(pos) = tokens.iter().position(|t| t.token_hash == token_hash) {
                Ok(Some(tokens.remove(pos)))
            } else {
                Ok(None)
            }
        }
        async fn delete_all_for_user(&self, user_id: Uuid) -> Result<(), UserError> {
            self.tokens.lock().await.retain(|t| t.user_id != user_id);
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockStateStore {
        inner: Mutex<HashMap<String, OAuthStateData>>,
    }

    #[async_trait::async_trait]
    impl OAuthStateStore for MockStateStore {
        async fn store(&self, state: &str, data: OAuthStateData) -> Result<(), OAuthStateError> {
            self.inner.lock().await.insert(state.to_string(), data);
            Ok(())
        }
        async fn take(&self, state: &str) -> Result<Option<OAuthStateData>, OAuthStateError> {
            Ok(self.inner.lock().await.remove(state))
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
            slug: &str,
            display_name: Option<&str>,
            is_personal: bool,
            created_by: Uuid,
        ) -> Result<Organization, OrganizationError> {
            let org = Organization {
                id: Uuid::new_v4(),
                slug: slug.into(),
                display_name: display_name.map(String::from),
                is_personal,
                created_by,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            self.orgs.lock().await.push(org.clone());
            Ok(org)
        }
        async fn find_by_id(
            &self,
            id: Uuid,
        ) -> Result<Option<Organization>, OrganizationError> {
            Ok(self.orgs.lock().await.iter().find(|o| o.id == id).cloned())
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
                .find(|o| o.created_by == user_id && o.is_personal)
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
    struct MockMemberRepo {
        members: Mutex<Vec<OrganizationMember>>,
    }

    #[async_trait::async_trait]
    impl OrganizationMemberRepository for MockMemberRepo {
        async fn add(
            &self,
            org_id: Uuid,
            user_id: Uuid,
            role: &str,
            title: Option<&str>,
            is_owner: bool,
            permissions: Permissions,
        ) -> Result<OrganizationMember, OrganizationMemberError> {
            let member = OrganizationMember {
                id: Uuid::new_v4(),
                org_id,
                user_id,
                role: role.into(),
                title: title.map(String::from),
                is_owner,
                permissions,
                joined_at: Utc::now(),
                updated_at: Utc::now(),
            };
            self.members.lock().await.push(member.clone());
            Ok(member)
        }
        async fn find_by_org_and_user(
            &self,
            _org_id: Uuid,
            _user_id: Uuid,
        ) -> Result<Option<OrganizationMember>, OrganizationMemberError> {
            Ok(None)
        }
        async fn list_by_org(
            &self,
            _org_id: Uuid,
        ) -> Result<Vec<OrganizationMember>, OrganizationMemberError> {
            Ok(vec![])
        }
        async fn list_by_user(
            &self,
            _user_id: Uuid,
        ) -> Result<Vec<OrganizationMember>, OrganizationMemberError> {
            Ok(vec![])
        }
        async fn update_role_and_title(
            &self,
            _org_id: Uuid,
            _user_id: Uuid,
            _role: &str,
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

    // --- Helpers -----------------------------------------------------------------

    fn test_jwt_config() -> JwtConfig {
        JwtConfig {
            secret: b"test-secret-for-unit-tests".to_vec(),
            access_expiry_secs: 900,
            refresh_expiry_secs: 2592000,
        }
    }

    /// Build an AuthService with mock repos, seeding the user repo with one user.
    /// Returns (service, user_id) for use in tests.
    async fn service_with_user() -> (
        AuthService<atproto_oauth::storage_lru::LruOAuthRequestStorage>,
        Uuid,
    ) {
        let user_repo = Arc::new(MockUserRepo::default());
        let user = user_repo
            .create_from_atproto("did:plc:testuser", Some("test.bsky.social"), None)
            .await
            .unwrap();

        let session_repo = Arc::new(MockSessionRepo::default());
        let refresh_repo = Arc::new(MockRefreshRepo::default());
        let state_store = Arc::new(MockStateStore::default());
        let oauth_storage = create_default_oauth_storage(NonZeroUsize::new(10).unwrap());

        let oauth_config = OAuthConfig {
            redirect_uri: "http://localhost:5173/callback".into(),
            client_id: "https://zurfur.app".into(),
            private_signing_key_data: atproto_identity::key::KeyData::new(
                atproto_identity::key::KeyType::P256Private,
                vec![0u8; 32],
            ),
            plc_hostname: "plc.directory".into(),
        };

        let org_repo = Arc::new(MockOrgRepo::default());
        let member_repo = Arc::new(MockMemberRepo::default());

        let service = AuthService::new(
            oauth_config,
            test_jwt_config(),
            oauth_storage,
            state_store,
            user_repo,
            session_repo,
            refresh_repo,
            org_repo,
            member_repo,
        );

        (service, user.id)
    }

    // --- Tests -------------------------------------------------------------------

    #[test]
    fn hash_token_is_deterministic() {
        let a = hash_token("my-refresh-token");
        let b = hash_token("my-refresh-token");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_token_differs_for_different_inputs() {
        let a = hash_token("token-a");
        let b = hash_token("token-b");
        assert_ne!(a, b);
    }

    #[test]
    fn hash_token_is_hex_sha256() {
        let hash = hash_token("hello");
        // SHA-256 produces 64 hex characters
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn refresh_session_with_valid_token() {
        let (service, user_id) = service_with_user().await;

        // Manually create a refresh token
        let raw_token = "test-refresh-token";
        let token_hash = hash_token(raw_token);
        let expires_at = Utc::now() + Duration::hours(24);
        service
            .refresh_repo
            .create(user_id, &token_hash, expires_at)
            .await
            .unwrap();

        let result = service.refresh_session(raw_token).await.unwrap();
        assert!(!result.access_token.is_empty());
        assert!(!result.refresh_token.is_empty());

        // Old token should be consumed (single-use)
        let second_attempt = service.refresh_session(raw_token).await;
        assert!(second_attempt.is_err());
    }

    #[tokio::test]
    async fn refresh_session_rejects_expired_token() {
        let (service, user_id) = service_with_user().await;

        let raw_token = "expired-token";
        let token_hash = hash_token(raw_token);
        let expires_at = Utc::now() - Duration::hours(1);
        service
            .refresh_repo
            .create(user_id, &token_hash, expires_at)
            .await
            .unwrap();

        let result = service.refresh_session(raw_token).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn refresh_session_rejects_unknown_token() {
        let (service, _user_id) = service_with_user().await;
        let result = service.refresh_session("nonexistent-token").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn logout_clears_refresh_tokens_and_session() {
        let (service, user_id) = service_with_user().await;

        // Add a refresh token and AT Protocol session
        let token_hash = hash_token("some-token");
        service
            .refresh_repo
            .create(user_id, &token_hash, Utc::now() + Duration::hours(24))
            .await
            .unwrap();

        let atproto_session = AtprotoSessionEntity {
            id: Uuid::new_v4(),
            user_id,
            did: "did:plc:testuser".into(),
            access_token: "at-token".into(),
            refresh_token: Some("at-refresh".into()),
            expires_at: Utc::now() + Duration::hours(1),
            pds_url: None,
        };
        service.session_repo.upsert(&atproto_session).await.unwrap();

        // Logout
        service.logout(user_id).await.unwrap();

        // Verify cleanup
        let token = service.refresh_repo.find_by_hash(&token_hash).await.unwrap();
        assert!(token.is_none());

        let session = service.session_repo.find_by_user_id(user_id).await.unwrap();
        assert!(session.is_none());
    }

    #[tokio::test]
    async fn issued_jwt_is_verifiable() {
        let (service, user_id) = service_with_user().await;

        let token = service
            .issue_access_jwt(&user_id, "did:plc:testuser", &Some("test.bsky.social".into()))
            .unwrap();

        let claims: ZurfurClaims =
            shared::jwt::verify(&token, &service.jwt_config.secret).unwrap();
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.did, "did:plc:testuser");
        assert_eq!(claims.handle, Some("test.bsky.social".into()));
    }
}
