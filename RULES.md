# RULES.md — Code Architecture Conventions

> **Updated 2026-04-14**
>
> For domain concepts and schema conventions, see `design/glossary.md`.
> This file covers code-level patterns and architectural rules.

## Layer Dependency Direction

```
api → application + persistence → domain + shared
```

- `domain` has no external dependencies. Pure entities, traits, errors.
- `application` depends on `domain` only. Sees repositories as `Arc<dyn Trait>`.
- `persistence` depends on `domain` + `sqlx`. Implements domain traits.
- `api` depends on everything. Wires services, handles HTTP.

**Never** add a dependency from `application` to `persistence`. Application code must not know about sqlx, pool types, or concrete repository structs.

## Persistence: Action Methods vs Persistence Methods

All SQL logic in the persistence layer falls into two categories:

### Persistence Methods

Executor-generic free functions that perform a single SQL operation. They accept `impl sqlx::Executor` — the caller decides whether that's a pool connection or a transaction. They are stateless building blocks.

```rust
async fn create_tag<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    category: TagCategory,
    name: &str,
    is_approved: bool,
) -> Result<Tag, TagError> {
    // Single INSERT — no opinion about transactions
}
```

### Action Methods

Compose persistence methods within a transaction boundary. They call `pool.begin()`, invoke persistence methods with `&mut *tx`, and commit. These are trait methods that perform multi-step atomic operations.

```rust
async fn create_and_attach(&self, ...) -> Result<Tag, TagError> {
    let mut tx = self.pool.begin().await?;
    let tag = create_tag(&mut *tx, ...).await?;
    attach_entity_tag(&mut *tx, ...).await?;
    increment_usage_count(&mut *tx, ...).await?;
    tx.commit().await?;
    Ok(tag)
}
```

### Rules

1. **Persistence methods are the default.** Every SQL operation should be an executor-generic free function first.
2. **Trait methods are thin wrappers.** They call a persistence method with `&self.pool`. They are "action methods with a scope of one."
3. **Action methods compose persistence methods.** They never write SQL directly — they call the same helpers that the trait methods use.
4. **The caller decides atomicity.** Persistence methods don't know or care if they're in a transaction. The action method (or future UoW) makes that decision.
5. **No SQL duplication.** If two methods run the same INSERT, extract it to a persistence method and call it from both.

## Repository Pattern

- Repository traits live in `domain/`. They define the contract.
- Implementations live in `persistence/src/repositories/`. One file per aggregate.
- Each file has a struct (`SqlxFooRepository`) with a `pool: Pool` field.
- `from_pool(pool) -> Arc<dyn Trait>` wraps the concrete type as a trait object.
- Application services receive `Arc<dyn Trait>` — never the concrete type.

## Error Handling

- All domain errors use `thiserror`.
- `AppError` in the API layer implements `IntoResponse` and returns JSON: `{"error": "...", "code": "..."}`.
- Service errors have `From` impls into `AppError` — handlers use `?` directly.
- Internal errors log the detail server-side and return a generic message to the client.

## Testing

- Application service tests use inline mock repos (`Mutex<Vec<T>>` storage).
- API integration tests use separate mock repos in `api/src/tests/`.
- Mocks implement the same domain traits — swapped in via `Arc<dyn Trait>`.
- Each commit must compile and pass tests independently.

## Cross-Aggregate Composition

Aggregates never reference each other in the schema. When operations span aggregates:

- **Phase 1 (current):** Action methods on repository traits handle atomic pairs (e.g., `create_and_attach` for tag + entity_tag).
- **Phase 2 (future):** Unit of Work — application layer starts a transaction and passes the executor down through persistence methods.
- **API-layer orchestration:** Best-effort side effects (e.g., org creation triggers tag + feed creation). Failures are logged, not propagated.
