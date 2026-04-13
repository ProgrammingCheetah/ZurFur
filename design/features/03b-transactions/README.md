> **Created 2026-04-13**

# Feature 3.5: Transaction Support

## Overview

Multi-step database operations that must succeed or fail together. The platform currently uses compensating rollbacks (create → attach fails → delete the created row). This is fragile — the rollback itself can fail, leaving orphaned data. Transaction support wraps related operations in a single database transaction so they are atomic.

This is infrastructure, not a product feature. It exists because the commission and payment critical path (Features 4 and 5) cannot tolerate partial state.

## Problem

Repositories are behind `Arc<dyn Trait>`. Each method acquires its own connection from the pool. There is no mechanism to share a transaction across multiple repository calls.

Current workarounds and their failure modes:

| Operation | Steps | Failure Mode |
|-----------|-------|-------------|
| Create entity tag | create tag → attach → increment count | Orphaned tag if attach fails (compensating delete can also fail) |
| Create system feed | create feed → attach to org | Orphaned feed if attach fails |
| Onboarding | create 3-4 feeds → attach each → mark complete | Partial feed set if any step fails; user marked complete with missing feeds |
| Commission creation (future) | create commission → create feed → attach → create slots | Partial commission visible to both parties |
| Payment (future) | create invoice → charge → record transaction → release payout | Money moved without record, or record without money |

The first three are tolerable — worst case is a few orphaned rows. The last two are not.

## Design

### Executor Abstraction

The core idea: repository methods should accept any SQLx executor (pool connection OR transaction), not just the pool. SQLx already supports this — both `Pool` and `Transaction` implement `sqlx::Executor`.

```rust
use sqlx::PgPool;

pub type Pool = PgPool;
pub type Tx = sqlx::Transaction<'static, sqlx::Postgres>;
```

### Phase 1: Composite Methods on Existing Repos

For operations that need atomicity today (tag + attach, feed + attach), add methods on the concrete `Sqlx*Repository` structs that handle the transaction internally. These are not exposed through the domain trait — they are persistence-layer conveniences.

```rust
impl SqlxTagRepository {
    /// Atomically create a tag and attach it to an entity.
    /// Runs in a single transaction — no orphaned rows on failure.
    pub async fn create_and_attach(
        &self,
        category: TagCategory,
        name: &str,
        is_approved: bool,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Tag, TagError> {
        let mut tx = self.pool.begin().await
            .map_err(|e| TagError::Database(e.to_string()))?;

        let tag = sqlx::query(/* INSERT INTO tag ... RETURNING ... */)
            .fetch_one(&mut *tx).await?;

        sqlx::query(/* INSERT INTO entity_tag ... */)
            .execute(&mut *tx).await?;

        sqlx::query(/* UPDATE tag SET usage_count = 1 */)
            .execute(&mut *tx).await?;

        tx.commit().await
            .map_err(|e| TagError::Database(e.to_string()))?;

        Ok(tag)
    }
}
```

Same pattern for `SqlxFeedRepository::create_and_attach`.

**Impact on application layer:**
- `TagService.create_entity_tag` calls `SqlxTagRepository::create_and_attach` directly instead of 3 separate repo calls + compensating rollback
- `FeedService.create_system_feed` calls `SqlxFeedRepository::create_and_attach` instead of 2 calls + compensating rollback
- Application services that need atomic operations hold the concrete `Sqlx*Repository` type (not `Arc<dyn Trait>`) for those specific methods

**Trade-off:** This breaks the clean trait abstraction for these methods. The composite methods are on the concrete struct, not the trait. Tests that mock the trait won't cover these paths. This is acceptable for Phase 1 — the operations are simple and the SQL is straightforward.

### Phase 2: Unit of Work

When commissions and payments arrive, composite methods won't scale — commission creation touches 4+ aggregates. A proper Unit of Work pattern is needed.

```rust
/// A database transaction scope. Repositories borrowed from a UoW
/// share the same underlying transaction.
pub struct UnitOfWork {
    tx: Option<Tx>,
}

impl UnitOfWork {
    /// Begin a new transaction.
    pub async fn begin(pool: &Pool) -> Result<Self, DbError> {
        let tx = pool.begin().await?;
        Ok(Self { tx: Some(tx) })
    }

    /// Commit the transaction. Returns error if already committed.
    pub async fn commit(mut self) -> Result<(), DbError> {
        let tx = self.tx.take()
            .ok_or(DbError::AlreadyCommitted)?;
        tx.commit().await?;
        Ok(())
    }

    /// Get a reference to the underlying executor for repo methods.
    /// Returns error if already committed.
    pub fn executor(&mut self) -> Result<&mut Tx, DbError> {
        self.tx.as_mut()
            .ok_or(DbError::AlreadyCommitted)
    }
}

// SQLx transactions roll back automatically on drop if not committed.
```

**Repository changes for Phase 2:**

Repository traits gain a second set of methods that accept an executor:

```rust
#[async_trait]
pub trait TagRepository: Send + Sync {
    // Existing: uses internal pool connection
    async fn create(&self, ...) -> Result<Tag, TagError>;

    // New: uses caller-provided executor (for UoW)
    async fn create_with(&self, tx: &mut Tx, ...) -> Result<Tag, TagError>;
}
```

Or, alternatively, the trait methods accept a generic executor from the start and the `&self` pool is used as the default:

```rust
#[async_trait]
pub trait TagRepository: Send + Sync {
    async fn create(&self, category: TagCategory, name: &str, is_approved: bool)
        -> Result<Tag, TagError>;
}

// The Sqlx impl internally decides: use self.pool or accept &mut Tx
```

**Decision deferred** to when Feature 4 is designed. The exact trait ergonomics depend on how many operations need UoW and whether we want compile-time safety (generic executor) or runtime flexibility (optional tx parameter).

### What NOT to Build

- **Distributed transactions** — not needed. All data is in one PostgreSQL instance.
- **Saga/choreography** — overkill for a monolith. Sagas are for microservice boundaries.
- **Automatic retry on serialization failure** — PostgreSQL default isolation (READ COMMITTED) doesn't have serialization conflicts for our workload.
- **Nested transactions** — SQLx savepoints exist but add complexity. Keep UoW flat.

## Operations That Need Transactions

### Phase 1 (Now — Tag/Feed Infrastructure)

| Operation | Aggregates | Current Approach | With Transactions |
|-----------|-----------|------------------|-------------------|
| Create entity tag | Tag + EntityTag | Compensating rollback | `create_and_attach` |
| Create system feed | Feed + EntityFeed | Compensating rollback | `create_and_attach` |

### Phase 2 (Feature 4 — Commissions)

| Operation | Aggregates | Notes |
|-----------|-----------|-------|
| Create commission | Commission + Feed + EntityFeed + Slots | All-or-nothing |
| Accept commission | Commission state + event feed item | State change + audit entry |
| Complete commission | Commission state + trigger payout | Must be atomic with payment |

### Phase 3 (Feature 5 — Payments)

| Operation | Aggregates | Notes |
|-----------|-----------|-------|
| Record payment | Transaction + Invoice status | Money tracking must be exact |
| Issue payout | Payout + Transaction | Cannot have payout without record |
| Process refund | Refund + Transaction + Commission state | Multi-table state change |

## Dependencies

### Requires
- Current repository infrastructure (Features 1-3)
- SQLx `pool.begin()` — already available, just unused

### Enables
- [Feature 4](../04-commission-engine/README.md) — atomic commission lifecycle
- [Feature 5](../05-financial-gateway/README.md) — atomic payment flows
- Removes compensating rollback fragility from Features 3 and 2

## Implementation Plan

### Phase 1 (Ship with or shortly after Feature 3)
1. Add `create_and_attach` to `SqlxTagRepository`
2. Add `create_and_attach` to `SqlxFeedRepository` (for system feeds)
3. Update `TagService.create_entity_tag` to use composite method
4. Update `FeedService.create_system_feed` to use composite method
5. Remove compensating rollbacks

### Phase 2 (Before Feature 4)
1. Design `UnitOfWork` struct in persistence crate
2. Add `_with(tx)` variants to repository traits that commissions need
3. Wire `UnitOfWork` creation into application services
4. Refactor `OnboardingService.complete_onboarding` as proof-of-concept
5. Document the pattern in `design/glossary.md`

## Assumptions

- Single PostgreSQL instance — no distributed transactions
- SQLx `pool.begin()` is the only primitive needed
- UoW is per-request, short-lived, not held across async boundaries unnecessarily
- Not every operation needs UoW — reads and single-row writes use the pool directly
- Phase 1 composite methods are an acceptable short-term deviation from the trait abstraction

## Shortcomings

- **Phase 1 composite methods bypass the trait** — not mockable in unit tests. Integration tests with a real DB are needed to cover these paths.
- **Phase 2 `_with(tx)` methods double the trait surface** — every transactional method needs a twin. This is ugly but explicit. Alternative: generic executor parameter, which is cleaner but harder with async traits.
- **UoW doesn't compose across services** — if Service A starts a UoW and calls Service B, B can't join A's transaction without being passed the UoW explicitly. This is fine for our architecture (services don't call each other) but would be a problem in a more layered design.
