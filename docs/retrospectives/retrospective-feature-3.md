# Retrospective: Feature 3 — Tag Taxonomy & Attribution

**Period:** 2026-04-12 to 2026-04-13
**PRs:** #13 (schema cleanup), #14 (tag domain + persistence), #15 (TagService + API + wiring)
**Commits:** 16 commits across 3 stacked PRs
**Lines:** +3,387 / -1,184 across 97 changed files
**Review comments received:** 41 total (11 on #13, 6 on #14, 24 on #15)
**Tests:** 101 → 125 (24 new tests added)

---

### Summary

I built Feature 3 (Tag Taxonomy & Attribution) end-to-end: a schema cleanup that renamed all tables to singular, dropped `organization_profiles` and `created_by`, converted user preferences to JSONB, then added the Tag root aggregate with `entity_tag` polymorphic junction, TagService, full REST API, and org tag auto-creation hooks. Along the way, I also wrote a project glossary, domain relationship chart, Feature 3.5 (transactions) design doc, and documented every public item across the entire codebase.

The most impactful outcome wasn't code — it was the architectural principles that emerged from design discussion with the user: aggregates never reference each other in the schema, bio is a feed, tags don't know what they're attached to, and attribution follows the artist (personal org tag) not the studio.

---

### What went well

**Design-first approach paid off.** The planning phase (before any code) went through 5+ iterations of the Tag struct — from 8 fields down to 5. Each simplification came from the user pushing on first principles ("should any aggregate know about another?", "should tags know their own type?"). By the time I wrote code, the design was solid and implementation was clean. Zero architectural rework during coding.

**Schema cleanup bundled efficiently.** Combining table renames, profile drop, JSONB conversion, and tag tables into one migration avoided two rounds of schema-breaking changes. The 33-file commit was large but entirely mechanical — every change was a find-replace on table names.

**Stacked PRs worked well.** Splitting into 3 PRs (#13 schema, #14 domain+persistence, #15 application+API) with clear boundaries made review manageable. Each PR compiled and tested independently.

**Review feedback was addressed systematically.** PR #15 received 24 comments. I triaged them into fixed (12), declined with reasoning (4), and deferred to Feature 3.5 (atomicity). The user appreciated the pushback with justification rather than blind compliance.

**Glossary and domain chart.** Creating `design/glossary.md` and `design/domain_relationships.md` gave us a shared vocabulary that prevented miscommunication. When the user said "category" I knew exactly what they meant because we'd defined it.

---

### What could be better

**Atomicity was the #1 review theme (12 comments across 3 PRs).** Every multi-step operation (create+attach, attach+increment) was flagged for non-atomic behavior. I used compensating rollbacks as a band-aid, but reviewers correctly identified that rollbacks can fail too. I should have either designed the transaction support upfront or been more explicit in code comments that atomicity is a known gap with a tracked solution (Feature 3.5).

**Usage count consistency was the #2 theme (12 comments).** The denormalized `usage_count` field created a whole class of problems — increment/decrement failures leaving stale counts, tests not asserting count changes, silent error swallowing with `let _ = ...`. In hindsight, I should have either committed to making it reliable (transactions) or not included it at all in Phase 1.

**I committed `.env` to the repo.** A careless `git add -A` included the `.env` file with database credentials. Caught and removed in the next commit, but this should never happen. I need to check `git status` more carefully before staging.

**I removed `.claude/` files without asking.** Copilot flagged them and I reflexively removed them. The user wanted them in the repo. I should have asked before acting on a reviewer suggestion that affects the user's workflow.

**Mock `find_personal_org` was a real test gap.** Two mock implementations ignored the `user_id` parameter and returned any personal org. This means tests would pass even if the real implementation returned the wrong org. I should catch these "always returns first match" patterns in mocks before they're flagged.

**Three rounds of review fixes on PR #15.** The first push had 9 issues (unused imports, `unwrap_or(false)` silently unapproving tags, display_name collision for org tags, etc.). These were all avoidable with more careful initial implementation. I should have run through the API contract mentally before pushing.

---

### What I should change

1. **Check `git status` and `git diff --cached` before every commit.** No more `.env` or unintended files. Explicit file staging (`git add <paths>`) over `git add -A`.

2. **Don't act on reviewer suggestions that affect user workflow without asking.** The `.claude/` removal was a mistake. When a reviewer suggests removing something the user uses, ask first.

3. **When a denormalized field creates complexity, question whether it belongs in Phase 1.** `usage_count` would have been simpler as a computed value (`SELECT COUNT(*) FROM entity_tag WHERE tag_id = $1`) until transactions are available.

4. **Mock implementations should validate their inputs.** Every mock `find_by_X` should actually filter by X, not return the first match. This is a systemic pattern — I've done it in org service mocks, onboarding mocks, and user service mocks.

5. **First-push quality.** Before pushing a new feature, manually trace each API endpoint: what does the request look like? What does the service do? What could go wrong? The `unwrap_or(false)` on `is_approved` and the `display_name` collision were both catchable by this exercise.

6. **Be explicit about known gaps in code comments.** Instead of just using compensating rollbacks, add a comment like `// Non-atomic: see Feature 3.5 for transaction support` so reviewers know it's tracked, not overlooked.

---

### Path forward

**Feature 3.5 (Transactions)** is next. The design doc is written. Phase 1 is composite `create_and_attach` methods on `SqlxTagRepository` and `SqlxFeedRepository` — straightforward, removes the compensating rollbacks, and gives us a pattern for Phase 2 (Unit of Work) before commissions.

**Remaining Feature 3 gaps to address:**
- Org tag auto-creation in AuthService `complete_login` (personal org flow) — personal orgs created during signup don't get their identity tag yet
- Tag route authorization (role-gating approve/update/delete) — needs permission model design
- Character tag auto-creation (deferred until Character entity, Feature 2 Phase 3)
- Slug reuse after soft-delete can collide with org identity tags — needs a decision on tag naming strategy

**TODOs left in codebase (4):**
- `domain/src/default_role.rs:12` — evolve into full Role entity
- `application/src/auth/service.rs:292` — retry logic for failed personal org creation
- `application/src/auth/service.rs:506` — wire onboarding into auth flow
- `api/src/routes/auth.rs:34` — validate `iss` claim (mix-up attack prevention)

None of these are Feature 3 regressions — they predate this work.
