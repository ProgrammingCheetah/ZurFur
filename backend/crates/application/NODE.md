---
path: backend/crates/application
charted: 2026-08-29
fs:
  - name: Cargo.toml
    role: crate manifest: deps on shared+domain+anyhow+tracing+serde_json; dev-deps test-support+tokio+uuid+chrono+async-trait
    node: false
  - name: src/lib.rs
    role: crate doc (the use-case shape contract) + module declarations, re-exports transaction()
    node: false
  - name: src/account.rs
    role: account module — AccountError, AccountPorts, and use cases list/create/delete/change_handle/accept_invitation/leave/grant_role/revoke_role
    node: false
  - name: src/user.rs
    role: user module — MeQuery/MeResult/MeError, the me use case
    node: false
  - name: src/commission.rs
    role: commission module — CommissionError, SweepResult, the sweep_deadlines use case
    node: false
  - name: src/transaction.rs
    role: the one transaction() begin/commit/rollback orchestrator (DD 24150017); mod, not pub mod
    node: false
  - name: tests/account.rs
    role: integration tests for account use cases against adapter-mem
    node: false
  - name: tests/commission.rs
    role: integration tests for sweep_deadlines
    node: false
  - name: tests/user.rs
    role: integration tests for the me use case
    node: false
  - name: tests/dep_guard.rs
    role: cargo-tree witness — application must never link an adapter, composition, or an HTTP stack
    node: false
---
**Is:** The application layer (DD 55836674): one plain async fn per use case, called by every driver (api, cli), holding orchestration between routing and domain — organized by entity module (account, user, commission) plus the shared `transaction()` orchestrator.

**Conventions:** one plain `pub async fn` per use case, no mediator. Shape: `Query`/`Command` DTO in → per-module `Ports` struct of `&dyn` ports → runtime config + `now: DateTimeUtc` as plain params → `Result<OutputDTO, <Module>Error>`. Output DTOs carry domain VALUES, never entities. One terse-`Display` error enum per module, never interpolates the cause (leak risk) — cause stays on `source()`. Writes call `transaction()`; reads skip the unit of work entirely. Modules are named after domain entities, not technical layers. Depends on `domain` and `shared` only — never an adapter or `composition`, enforced by `tests/dep_guard.rs`. A use case need not have an actor — `commission::sweep_deadlines` is the system acting on an injected `now`.

**Entry points:** `src/lib.rs` · `src/account.rs` · `src/user.rs` · `src/commission.rs` · `src/transaction.rs`.

**Refs:** DD 55836674 — The Application Layer, Use Cases, DTOs and Ports · DD 24150017 — Transactions as a capability · DD 23003138 — Account Deletion, Tombstoning & Handle Reuse · memory `project_application_layer_convention.md` · memory `project_transaction_unit_of_work.md` · memory `feedback_work_by_module.md`.
