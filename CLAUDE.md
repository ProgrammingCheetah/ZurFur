# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Zurfur is an AT Protocol-native art commission platform built in Rust. It uses Bluesky OAuth for authentication and is designed as a headless backend with a future frontend.

## Commands

All commands use `just` (Justfile at repo root). The Justfile has `dotenv-load` enabled.

```bash
just dev                   # Start everything: Docker, DB migrations, backend + auth frontend
just up                    # Start PostgreSQL + Nginx via Docker Compose
just down                  # Stop containers
just dev-back              # cargo watch -x run (from backend/)
just dev-auth              # Vite dev server for frontend/auth
just check                 # bacon (background type checker, from backend/)
just db-shell              # psql into the running database
just migrate-add <name>    # Create new migration pair
just migrate-run           # Run pending migrations
just db-reset              # Drop, recreate, and migrate database
just setup                 # First-time setup: copy .env, install tools, yarn install
```

Building and running directly:
```bash
cd backend && cargo build            # Build all crates
cd backend && cargo run -p api       # Run the API server
cd backend && cargo test             # Run all tests
cd backend && cargo test -p domain   # Test a single crate
```

## Architecture

Layered DDD with strict dependency direction: `api` -> `application` + `persistence` -> `domain` + `shared`.

```
backend/crates/
  domain/        # Pure entities, repository traits, errors (no I/O, no dependencies)
  shared/        # JWT utilities, config structs
  persistence/   # SQLx PostgreSQL implementations of domain traits, migrations
  application/   # Use-case orchestrators (AuthService, login flow)
  api/           # Axum HTTP handlers, middleware, routing
```

**Key conventions:**
- Repository traits live in `domain/`, implementations in `persistence/`
- Application services receive repositories as trait objects (`Arc<dyn Trait>`)
- All error types use `thiserror`
- Rust edition 2024 across all crates
- Workspace-level dependency versions in root `Cargo.toml`

## Authentication Flow

AT Protocol OAuth (not password-based):
1. `POST /auth/start` - resolves handle/DID, initiates OAuth with PAR + PKCE + DPoP
2. `POST /auth/callback` - frontend-mediated; exchanges code for AT Protocol tokens, creates/finds user, issues Zurfur JWT + refresh token
3. `POST /auth/refresh` - single-use rotation (old token deleted, new pair issued)
4. `POST /auth/logout` - deletes all refresh tokens + AT Protocol session

Zurfur refresh tokens are SHA-256 hashed before storage. JWTs are HS256 with 15-min default TTL.

## Database

PostgreSQL 16 via Docker Compose (port 5432, user: admin, db: zurfur). Migrations live in `backend/crates/persistence/migrations/` and auto-run on startup. Uses soft deletes (`deleted_at`) for users.

## Environment Variables

Required: `DATABASE_URL`, `JWT_SECRET`, `OAUTH_CLIENT_ID`, `OAUTH_REDIRECT_URI`, `OAUTH_PRIVATE_KEY` (base64 P256 key).

Optional with defaults: `JWT_ACCESS_EXPIRY_SECS` (900), `JWT_REFRESH_EXPIRY_SECS` (2592000).

## Design Documents

- `design/design_document.md` - Full platform architecture and feature specs
- `design/features/OVERVIEW.md` - Feature dependency map and build order
- `diagrams/auth/login.mermaid` - OAuth login sequence diagram

## Retrospectives

Read `docs/retrospectives.md` before starting any feature work. Contains shared guidance on branching, testing, commit discipline, automated reviewer patterns, common pitfalls, and per-feature lessons learned.
