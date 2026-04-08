//! AT Protocol OAuth login flow: identity resolution, PAR, token exchange, and session.

use std::num::NonZeroUsize;
use std::sync::Arc;

use atproto_identity::key::{KeyType, generate_key, to_public};
use atproto_identity::model::Document;
use atproto_identity::resolve::{
    HickoryDnsResolver, InnerIdentityResolver, SharedIdentityResolver,
};
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::errors::OAuthClientError;
use atproto_oauth::pkce;
use atproto_oauth::resources::{oauth_authorization_server, pds_resources};
use atproto_oauth::storage::OAuthRequestStorage;
use atproto_oauth::storage_lru::LruOAuthRequestStorage;
use atproto_oauth::workflow::{
    OAuthClient, OAuthRequest, OAuthRequestState, ParResponse, oauth_complete, oauth_init,
};
use chrono::{Duration, Utc};
use reqwest::header::{HeaderMap, HeaderValue};
use thiserror::Error;

// --- Errors ------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum LoginError {
    #[error("Invalid email")]
    InvalidEmail,
    #[error("User not found")]
    UserNotFound,
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("Identity resolution failed")]
    IdentityResolverFailed,
    #[error("No PDS found for account")]
    PdsNotFound,
    #[error("OAuth error: {0}")]
    OAuth(String),
    #[error("Invalid or expired state")]
    InvalidState,
    #[error("Token response DID does not match resolved identity")]
    DidMismatch,
}

/// Session tokens after OAuth completion; store these and use access_jwt for API calls.
#[derive(Debug, Clone)]
pub struct AtprotoSession {
    pub did: String,
    pub handle: Option<String>,
    pub email: Option<String>,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in_secs: u32,
}

// --- OAuth start result ------------------------------------------------------

/// Result of starting the OAuth flow: redirect the user to this URL.
#[derive(Debug, Clone)]
pub struct OAuthStartResult {
    pub redirect_url: String,
    pub state: String,
}

// --- Configuration ----------------------------------------------------------

/// Configuration for the AT Protocol OAuth client (redirect URI, client ID, signing key).
#[derive(Clone)]
pub struct OAuthConfig {
    pub redirect_uri: String,
    pub client_id: String,
    /// Private key for client assertions (confidential client). Generate once with
    /// `atproto_identity::key::{generate_key, KeyType::P256Private}`.
    pub private_signing_key_data: atproto_identity::key::KeyData,
}

// --- Identity resolver (reqwest 0.12 for atproto crates) ---------------------

async fn build_http_client() -> Result<reqwest::Client, LoginError> {
    let user_agent = HeaderValue::from_str("Zurfur/0.0.1")
        .map_err(|_| LoginError::InternalError("Invalid User-Agent".into()))?;
    let mut headers = HeaderMap::new();
    headers.append("User-Agent", user_agent);
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| LoginError::InternalError(e.to_string()))
}

async fn get_identity_resolver() -> Result<SharedIdentityResolver, LoginError> {
    let http_client = build_http_client().await?;
    let dns_resolver = Arc::new(HickoryDnsResolver::create_resolver(&[]));
    let inner = InnerIdentityResolver {
        dns_resolver,
        http_client,
        plc_hostname: "https://plc.directory".into(),
    };
    Ok(SharedIdentityResolver(Arc::new(inner)))
}

async fn resolve_identity(handle_or_did: &str) -> Result<Document, LoginError> {
    let resolver = get_identity_resolver().await?;
    resolver
        .resolve(handle_or_did)
        .await
        .map_err(|_| LoginError::IdentityResolverFailed)
}

fn pds_from_document(doc: &Document) -> Result<&str, LoginError> {
    doc.pds_endpoints()
        .first()
        .map(|s| *s)
        .ok_or(LoginError::PdsNotFound)
}

// --- Key serialization for OAuth request storage ------------------------------

fn serialize_key_data(key: &atproto_identity::key::KeyData) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(key.bytes())
}

fn deserialize_dpop_key(s: &str) -> Result<atproto_identity::key::KeyData, LoginError> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| LoginError::InternalError(format!("Invalid stored DPoP key: {}", e)))?;
    Ok(atproto_identity::key::KeyData::new(
        KeyType::P256Private,
        bytes,
    ))
}

// --- OAuth start (PAR + redirect URL) ----------------------------------------

/// Start the AT Protocol OAuth flow: run PAR using a pre-resolved identity document,
/// store state, and return a redirect URL.
pub async fn start_oauth_login(
    handle_or_did: &str,
    document: &Document,
    config: &OAuthConfig,
    storage: &impl OAuthRequestStorage,
) -> Result<OAuthStartResult, LoginError> {
    let http_client = build_http_client().await?;
    let pds = pds_from_document(document)?;
    let (_resource, auth_server) = pds_resources(&http_client, pds)
        .await
        .map_err(|e| LoginError::OAuth(e.to_string()))?;

    let (pkce_verifier, code_challenge) = pkce::generate();
    let state = uuid::Uuid::new_v4().to_string();
    let nonce = uuid::Uuid::new_v4().to_string();
    let scope = "atproto transition:generic".to_string();

    let dpop_key =
        generate_key(KeyType::P256Private).map_err(|e| LoginError::InternalError(e.to_string()))?;

    let oauth_client = OAuthClient {
        redirect_uri: config.redirect_uri.clone(),
        client_id: config.client_id.clone(),
        private_signing_key_data: config.private_signing_key_data.clone(),
    };

    let oauth_request_state = OAuthRequestState {
        state: state.clone(),
        nonce: nonce.clone(),
        code_challenge,
        scope: scope.clone(),
    };

    let login_hint = Some(handle_or_did);
    let par_response: ParResponse = oauth_init(
        &http_client,
        &oauth_client,
        &dpop_key,
        login_hint,
        &auth_server,
        &oauth_request_state,
    )
    .await
    .map_err(|e: OAuthClientError| LoginError::OAuth(e.to_string()))?;

    let now = Utc::now();
    let expires_at = now + Duration::minutes(10);

    let oauth_request = OAuthRequest {
        oauth_state: state.clone(),
        issuer: auth_server.issuer.clone(),
        authorization_server: auth_server.issuer.clone(),
        nonce,
        pkce_verifier,
        signing_public_key: serialize_key_data(
            &to_public(&config.private_signing_key_data)
                .map_err(|e| LoginError::InternalError(format!("Failed to derive public key: {}", e)))?,
        ),
        dpop_private_key: serialize_key_data(&dpop_key),
        created_at: now,
        expires_at,
    };

    storage
        .insert_oauth_request(oauth_request)
        .await
        .map_err(|e| LoginError::InternalError(format!("Failed to store OAuth state: {}", e)))?;

    let redirect_url = format!(
        "{}?client_id={}&request_uri={}",
        auth_server.authorization_endpoint,
        urlencoding::encode(&oauth_client.client_id),
        urlencoding::encode(&par_response.request_uri),
    );

    Ok(OAuthStartResult {
        redirect_url,
        state,
    })
}

// --- OAuth complete (exchange code, verify sub, return session) ---------------

/// Complete the OAuth flow: validate state, exchange code for tokens, verify `sub` matches expected DID.
pub async fn complete_oauth_login(
    code: &str,
    state: &str,
    expected_did: &str,
    handle: Option<&str>,
    config: &OAuthConfig,
    storage: &impl OAuthRequestStorage,
) -> Result<AtprotoSession, LoginError> {
    let http_client = build_http_client().await?;

    let oauth_request = storage
        .get_oauth_request_by_state(state)
        .await
        .map_err(|e| LoginError::InternalError(e.to_string()))?
        .ok_or(LoginError::InvalidState)?;

    storage.delete_oauth_request_by_state(state).await.ok();

    let auth_server = oauth_authorization_server(&http_client, &oauth_request.issuer)
        .await
        .map_err(|e| LoginError::OAuth(e.to_string()))?;

    let dpop_key = deserialize_dpop_key(&oauth_request.dpop_private_key)?;

    let oauth_client = OAuthClient {
        redirect_uri: config.redirect_uri.clone(),
        client_id: config.client_id.clone(),
        private_signing_key_data: config.private_signing_key_data.clone(),
    };

    let token_response = oauth_complete(
        &http_client,
        &oauth_client,
        &dpop_key,
        code,
        &oauth_request,
        &auth_server,
    )
    .await
    .map_err(|e: OAuthClientError| LoginError::OAuth(e.to_string()))?;

    let sub = token_response
        .sub
        .as_deref()
        .ok_or(LoginError::DidMismatch)?;
    if sub != expected_did {
        return Err(LoginError::DidMismatch);
    }

    Ok(AtprotoSession {
        did: sub.to_string(),
        handle: handle.map(String::from),
        email: token_response
            .extra
            .get("email")
            .and_then(|v| v.as_str())
            .map(String::from),
        access_token: token_response.access_token,
        refresh_token: token_response.refresh_token,
        expires_in_secs: token_response.expires_in,
    })
}

/// Resolve handle or DID to the full identity document. Returns the DID document
/// which can be passed to `start_oauth_login` to avoid redundant resolution.
pub async fn resolve_identity_document(handle_or_did: &str) -> Result<Document, LoginError> {
    resolve_identity(handle_or_did).await
}

// --- LRU storage constructor --------------------------------------------------

/// Create an in-memory OAuth request storage (state, PKCE, etc.) with the given capacity.
pub fn default_oauth_storage(capacity: NonZeroUsize) -> LruOAuthRequestStorage {
    LruOAuthRequestStorage::new(capacity)
}

