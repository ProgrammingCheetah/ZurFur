# Retrospective: Feature 3.5 — Transaction Support

**Period:** 2026-04-14
**PRs:** #16 (code review cleanup), #17 (Phase 1 — create_and_attach), #18 (Phase 2 — remaining atomicity)
**Commits:** 21 commits across 3 PRs
**Lines:** +665 / -205 on Phase 2 alone; ~900 net across all three PRs
**Review comments received:** 3 from Copilot on #16, 9 from user + 8 from Copilot on #18, 3 from user on #17
**Tests:** Held steady at 125 throughout (no new tests needed — existing coverage exercised new code paths)

---

### Summary

I built Feature 3.5 (Transaction Support) end-to-end across two phases. Phase 1 introduced `create_and_attach` composite methods on TagRepository and FeedRepository, replacing compensating rollbacks with proper SQL transactions. Phase 2 extended the pattern to all remaining non-atomic operations: tag attach/detach + usage_count, org + owner member creation, and feed item + elements creation. Along the way, I established the "Action vs Persistence method" convention, created RULES.md, and extracted 10 executor-generic persistence helpers across 6 repository files.

The most impactful outcome was the architectural pattern that emerged from user feedback on PR #17: the user pushed back on SQL duplication in `create_and_attach`, which led to the executor-generic free function pattern. This turned a Phase 1 shortcut into a principled approach that made Phase 2 trivial — every new action method was just "compose existing helpers with a transaction."

---

### What went well

**User-driven architecture improvement.** The initial `create_and_attach` implementations duplicated SQL inline. The user's PR #17 review ("Shouldn't these be modular?", "We should just pass the transaction rather than the pool, right?") drove the refactor to executor-generic helpers. This was the right call — it eliminated duplication and established a pattern that Phase 2 followed mechanically. The user named the pattern ("Action vs Persistence methods") and I codified it in RULES.md.

**Modular commits enabled smooth review.** The user explicitly asked for commits they could review individually. Phase 1 had 9 commits, Phase 2 had 8. Each compiled and tested independently. The user reviewed commit-by-commit on GitHub and left targeted comments. This was much smoother than the Feature 3 experience (3 rounds of fixes on PR #15).

**First-push quality improved.** Phase 2's core commits (1-4) landed with only style feedback — no logic bugs, no missing error variants, no incorrect SQL. The Copilot-caught issues (double-wrapped error strings, ambiguous NotFound) were real but subtle. Compare this to Feature 3 where the first push had 9 issues including silently unapproving tags.

**Responsive to review.** The user left 9 comments on PR #18 and I addressed all of them in a single follow-up commit (`8d4f2c0`). Copilot left 4 comments and I addressed those in another commit (`6fc6f94`). The comment-fix-push cycle was tight.

**RULES.md created.** The project now has documented code conventions that persist across sessions. This was user-initiated ("We should make a RULES.md") and fills a real gap — the glossary covered domain concepts but not code patterns.

---

### What could be better

**Mock complexity was the #1 review theme (5 of 9 user comments).** The shared `Arc<Mutex<Vec<EntityTag>>>` state between MockTagRepo and MockEntityTagRepo was "very convoluted" per the user. I used `drop()` instead of block scopes, built structs inline instead of using `::new()` constructors, and had unnecessary underscore prefixes on used parameters. All of these are Rust idiom issues — I should know better by now.

**Underscore prefix habit persists.** The user flagged `_entity_type` being used on the next line. This is the same category of careless error from Feature 3 (unused imports, `unwrap_or(false)`). I'm pattern-matching from function signatures where params are genuinely unused, then copy-pasting the underscores into implementations where they're used. I need to actively check: "is this parameter actually used in the body?"

**Error variant ambiguity missed.** I reused `TagError::NotFound` for "tag not attached to entity" in `detach_and_decrement`, even though `TagServiceError::NotAttached` already existed at the service layer. Copilot caught this. I should have noticed the semantic mismatch — the domain error layer should be at least as precise as the service error layer.

**Double-wrapped error strings.** `EntityTagError::Database("foo").to_string()` produces `"Database error: foo"`, and wrapping that in `TagError::Database(...)` doubles the prefix. This is a pattern I've hit before (similar to the Feature 3 `to_string()` on errors). I should destructure `Database(msg)` variants explicitly rather than falling through to `other.to_string()`.

**API mock inconsistency.** I fixed shared state in the application-level mocks but forgot to do the same in the API-level mocks. Copilot caught this. When a pattern change affects both mock locations, I need to update both in the same commit.

---

### What I should change

1. **Always use `::new()` constructors on mock structs.** Inline struct construction with shared state is hard to read. A constructor makes the dependency explicit: `MockTagRepo::new(shared_entity_tags.clone())`.

2. **Block scopes over `drop()` for lock release.** The user agreed after I explained both approaches — block scopes give compile-time safety while `drop()` is a runtime-only guard. Default to blocks.

3. **Check underscore prefixes before committing.** Grep for `_[a-z]` in the diff and verify each one is genuinely unused. This is a recurring mistake.

4. **Destructure error variants explicitly.** Never write `other => TagError::Database(other.to_string())` when the `other` variant is `Database(msg)` — destructure it: `EntityTagError::Database(msg) => TagError::Database(msg)`.

5. **Update both mock locations in the same commit.** When a trait changes, grep for all `impl Trait for Mock` locations and update them together. Don't leave the API mocks inconsistent.

6. **Document trade-offs in code comments.** The user explicitly asked for a comment on the extra `list_by_org` query in `create_org`. When making a deliberate trade-off (simplicity vs performance, extra query vs trait complexity), leave a comment so future readers understand why.

---

### Path forward

**Feature 3.5 is functionally complete.** All single-aggregate multi-step operations are now transactional. Two cross-service operations remain deferred with clear TODOs:
- Onboarding (feeds + mark complete) — recoverable via idempotency, needs UoW
- API orchestration (org + tag + feed) — best-effort by design, needs cross-service UoW

**UoW design deferred to Feature 4.** The design doc specifies this explicitly: "Decision deferred to when Feature 4 is designed." The executor-generic helpers are ready — the UoW just needs to provide a way for the application layer to start a transaction and thread it down.

**FIXME remaining:** `increment/decrement_usage_count` should check `rows_affected` for defensive consistency. Currently safe because callers verify existence, but this should be fixed when we next touch these helpers.

**Next feature in the critical path:** Feature 2 Phase 3 (Characters) or Feature 11 (Org TOS), depending on priority. Both are prerequisites for Feature 4 (Commission Engine).

**Rust confidence update:** Transaction work went well. The `pool.begin()` + `&mut *tx` + executor-generic pattern is now comfortable. The weak spots are mock ergonomics (shared state, constructors) and error variant precision — not lifetime/borrowing issues as I initially feared.
