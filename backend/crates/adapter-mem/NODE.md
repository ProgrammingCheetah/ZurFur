---
path: backend/crates/adapter-mem
charted: 2026-08-29
fs:
  - name: Cargo.toml
    role: domain + async-trait + cid/sha2 (real CIDv1 addressing in the fake)
    node: false
  - name: src/lib.rs
    role: shared MemBackend + MemDatabase/MemUnitOfWork; MemUserStore/Writes, MemAccountStore/Writes, MemAuthenticator, MemProfileSource, MemProfileCache, MemDidMinter, MemKeyStore, MemPlcOperationLog
    node: false
  - name: src/actor_identity.rs
    role: MemActorIdentityStore/Writes + StoredActorIdentity
    node: false
  - name: src/commission.rs
    role: commission/slot/seat/tab/element/changelog fakes — MemCommissionStore/Writes, MemChangelogStore/Writes + Stored* rows
    node: false
  - name: src/file_store.rs
    role: MemFileStore (CommissionFile port fake)
    node: false
  - name: src/public_records.rs
    role: MemPublicRecords — content-addressed (CIDv1/sha2-256) fake of the PDS boundary
    node: false
  - name: tests/commission.rs
    role: commission port conformance run against the mem fakes
    node: false
  - name: tests/public_records.rs
    role: shared PublicRecords conformance suite (test-support::contract) run against MemPublicRecords
    node: false
---
**Is:** In-process fakes of every domain port (private + public store) so core development and most tests need neither a database nor a PDS, reproducing each contract — including transactional rollback — without the operational reality.

**Conventions:** fidelity, not realism — reproduce the contract a handler depends on (idempotent recognition, soft-delete invisibility, cache hits); skip TTLs and real keypairs; call out intentional divergence on the item. Read/write split mirrors adapter-pg exactly: one shared `MemBackend` owns the maps; read stores read `&self`; write views are reachable only on a `MemUnitOfWork` vended by `MemDatabase`. `MemDatabase::begin` takes two independent deep copies (base + staged); `MemUnitOfWork::commit` diffs key-by-key and merges only this unit's changes back — not a wholesale replace; drop without commit discards both (rollback). The read-through `MemProfileCache` is the one Unit-of-Work exemption: its best-effort fill writes straight to the shared store, neither staged, merged, nor rolled back. Locking: `std::sync::Mutex`, never held across `.await` — each method locks, does sync map work, drops the guard, then returns; poisoned lock is unrecoverable so `.lock().expect()` throughout. Call counters use `AtomicUsize`, no lock needed.

**Entry points:** `src/lib.rs`.

**Refs:** DD "Transactions as a capability — compile-enforced Unit of Work" (24150017) · memory `project_transaction_unit_of_work`.
