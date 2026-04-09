# Feature 1: AT Protocol Auth & Bluesky Integration

## Overview

The foundational authentication and social layer of Zurfur. Users authenticate exclusively through their existing Bluesky/AT Protocol identity (no standalone accounts). This feature also covers bi-directional data sync, social graph import, and native Bluesky feed/DM integration within the Zurfur UI.

## Sub-features

### 1.1 Frictionless Onboarding (Bluesky OAuth)

**Status:** IMPLEMENTED

**What it is:** Users log in via Bluesky OAuth. Their Decentralized Identifier (DID) becomes their Zurfur identity. No traditional registration flow.

**Implementation:** Full AT Protocol OAuth with PAR, PKCE, DPoP, token exchange, DID validation. `AuthService` orchestrator in the application layer. JWT access tokens (15 min) with refresh token rotation (30 days). Pluggable `OAuthStateStore` trait for swap to Redis in production.

**Key files:**
- `backend/crates/application/src/auth/login.rs` — Core OAuth flow
- `backend/crates/application/src/auth/service.rs` — AuthService orchestrator
- `backend/crates/api/src/routes/auth.rs` — HTTP endpoints
- `backend/crates/api/src/middleware/auth.rs` — JWT extractor
- `backend/crates/domain/src/oauth_state_store.rs` — Pluggable state storage trait

### 1.2 Bi-Directional Data Sync

**What it is:** Artists can cross-post commission openings, completed artwork, and status updates directly to their Bluesky feed from the Zurfur dashboard.

**Implementation approach:**
- **Bluesky client trait** in domain layer (`BlueskyClient`) abstracting XRPC calls
- **XRPC implementation** in persistence/infrastructure using stored AT Protocol tokens from `atproto_sessions`
- **AT Protocol token refresh** — tokens expire ~1-2h; implement automatic refresh using stored `refresh_token` before making API calls
- **Cross-post use cases** in application layer: `post_commission_opening`, `post_completed_artwork`, `post_status_update`
- XRPC call: `com.atproto.repo.createRecord` targeting `app.bsky.feed.post` collection
- Optional image embedding via `com.atproto.repo.uploadBlob`

### 1.3 Social Graph Import

**What it is:** On login, the platform reads the user's AT Protocol social graph, recognizing existing Follows and Mutuals without rebuilding from scratch.

**Implementation approach:**
- XRPC calls to `app.bsky.graph.getFollows` and `app.bsky.graph.getFollowers` (paginated via cursor)
- New domain entity: `SocialConnection { user_id, target_did, relationship: Follow|Follower|Mutual }`
- New migration: `social_connections` table
- Cross-reference fetched DIDs with existing Zurfur users to identify on-platform connections
- Trigger: on first login (onboarding) or manually via API
- Periodic background sync to catch new follows

> **Architecture note:** The `social_connections` table serves as a staging table for the initial import. In Phase 2+, imported follows should be mapped to feed subscriptions on the corresponding personal org's default feed. The long-term social graph model is feed subscriptions (see Feature 2.3), not a separate connections table.

### 1.4 Native Social Integration

**What it is:** Bluesky feeds and DMs rendered natively within the Zurfur UI, making it function as a full social client alongside the commission tools.

**Implementation approach:**
- **Feeds:** Proxy `app.bsky.feed.getTimeline` and `app.bsky.feed.getAuthorFeed` through Zurfur API. Aggressive caching (short TTL). Frontend renders Bluesky post format.
- **DMs:** AT Protocol uses `chat.bsky.convo.*` lexicons. Start with polling, upgrade to WebSocket/SSE. Proxy through Zurfur API to avoid exposing AT Protocol tokens to frontend.
- **Posting:** `com.atproto.repo.createRecord` for new posts from within Zurfur
- New API routes: `GET /social/feed`, `GET /social/feed/:did`, `GET /social/messages`, `POST /social/messages`

## Dependencies

### Requires (must be built first)
- None — this is the foundational feature
- 1.2 soft-depends on [Feature 3](../03-commission-engine/README.md) — commission entities needed to cross-post commission-specific content
- 1.3 and 1.4 depend on 1.1 being complete (need stored AT Protocol tokens)

### Enables (unlocked after this is built)
- [Feature 2](../02-identity-profile/README.md) — profiles require authenticated users
- [Feature 3](../03-commission-engine/README.md) — commissions require authenticated users
- [Feature 8](../08-search-discovery/README.md) — search requires users to exist
- [Feature 9](../09-notification-system/README.md) — notifications require knowing who to notify
- Essentially every other feature depends on 1.1

## Implementation Phases

### Phase 1: OAuth Foundation (DONE)
- AuthService with start_login, complete_login, refresh_session, logout
- JWT middleware (AuthUser extractor)
- Refresh token rotation with SHA-256 hashing
- Pluggable OAuthStateStore trait + InMemoryOAuthStateStore
- User entity with did, handle, email, and username fields
- Database: users (did, handle, email, username columns), atproto_sessions, refresh_tokens tables
- After user creation during OAuth callback, a personal organization is automatically created for the user (implemented in Feature 2 Phase 1)

### Phase 2: Bluesky Client & Sync
- BlueskyClient trait in domain layer
- XRPC client implementation using reqwest + stored AT Protocol tokens
- AT Protocol token auto-refresh mechanism
- Social graph import (1.3)
- Generic "post to Bluesky" use case
- Cross-post endpoint: `POST /bluesky/post`

### Phase 3: Native Integration & Post-implementation
- Feed proxy endpoints (1.4)
- DM proxy with WebSocket/SSE
- Integration tests against Bluesky sandbox/staging
- Rate limiting on auth and Bluesky proxy endpoints
- Monitoring: track OAuth success/failure rates, token refresh failures
- Documentation: OAuth flow sequence diagram for frontend developers

## Assumptions

- AT Protocol OAuth spec remains stable at v0.14.x
- Bluesky's XRPC endpoints (`app.bsky.*`) maintain backward compatibility
- All Zurfur users have Bluesky accounts — no standalone account creation
- In-memory OAuth state storage is acceptable for development (Redis for production)
- Bluesky chat API (`chat.bsky.convo.*`) is accessible without special permissions

## Shortcomings & Known Limitations

- **OAuth state is in-memory (LRU):** Server restart between login start and callback loses state. `OAuthStateStore` trait is the seam for Redis swap.
- **AT Protocol token refresh not yet automated:** Tokens expire ~1-2h. Must be built for 1.2-1.4.
- **No rate limiting** on auth endpoints — vulnerable to brute force
- **DM integration (1.4)** may have Bluesky access restrictions not yet known
- **No fallback** if Bluesky is down — users cannot log in at all
- **Single PDS assumption:** Code resolves the first PDS endpoint; users with multiple PDS endpoints untested
