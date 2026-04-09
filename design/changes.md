# Planned Changes to Design Documents

Based on the inconsistencies review and owner solutions (2026-04-09).

---

## Cross-Cutting Changes

### A. Feed model update — add `feed_elements` table
**Affects:** Feature 2 (canonical definition), Feature 3, Feature 5, Feature 6, Feature 9, design_document.md

Current `feed_items` has `item_type` + `content_json` trying to be everything. Split into:

```
feed_items: id, feed_id, author_type, author_id, created_at
feed_elements: id, feed_item_id, element_type (text/image/file/event/embed/...), content_json, position
```

A feed item can have multiple elements (text + image + file in one post). Remove `item_type` and `content_json` from `feed_items`. Update all references.

### B. Roles as free-text with default_roles reference table
**Affects:** Feature 2 (org_members schema), Feature 3 (commission participants), design_document.md

Replace `role ENUM` with `role TEXT` on `org_members`. Add:

```
default_roles: id, name (owner/admin/mod/member), default_permissions (BIGINT), hierarchy_level (INT)
```

System roles (owner/admin/mod/member) have meaning for permission defaults and hierarchy. Custom roles ("Lead Colorist") are just display strings. Onboarding sets a role from `default_roles`; users can customize later.

### C. Remove `commission_status` everywhere — use tags
**Affects:** Feature 2 (org profile), Feature 3, Feature 7, Feature 8, design_document.md

No `commission_status` column on orgs or org profiles. Commission availability is expressed through tags on the org (`status:open`, `status:closed`, `status:waitlist`). Bio is human-readable context. The commissions feed's existence signals the org does commissions.

### D. Commission states are artist-defined, not a fixed enum
**Affects:** Feature 3 (major), design_document.md (state diagram)

Replace `internal_state ENUM (Blocked/InProgress/AwaitingInput/Completed)` with `current_state TEXT` validated against the pipeline template's state list. Templates define valid states, transitions, and which states are terminal. System only cares about active vs terminal.

### E. Commission → feed relationship inverted
**Affects:** Feature 3, Feature 5

Commission entity has NO feed field. Feeds know about commissions via `entity_feeds`. Same pattern as all other entities. Remove any `feed` field from Commission.

### F. `feed_subscriptions` moved to Feature 2
**Affects:** Feature 2 (add definition), Feature 6 (remove duplicate, reference Feature 2)

```
feed_subscriptions: id, feed_id, subscriber_org_id, permissions, granted_at, granted_by_user_id
```

Defined in Feature 2 since feeds are foundational. Feature 6 references it.

### G. Feeds table updated
**Affects:** Feature 2 (canonical definition), design_document.md

```
feeds: id, slug, display_name, description, feed_type (system/custom), created_at, updated_at, deleted_at
```

Added: `description`, `updated_at`, `deleted_at` (soft-delete). No `created_by` or owner — ownership is via `entity_feeds` relationship. Permissions flow through the owning entity (org admin manages org feeds).

### H. Feature 1 dependency cleanup
**Affects:** Feature 1 (Enables section), OVERVIEW.md (graph edges)

Remove direct F1→F3, F1→F9 edges. F1 enables F2, everything else gets auth transitively. Add a note: "All features require authentication (Feature 1) transitively through Feature 2."

---

## Per-File Changes

### design_document.md

1. Update Part 2.5.1 root aggregates table — Feed description now includes `feed_elements`
2. Update Part 2.5.3 — remove `commission_status` from org settings list, replace with "tags determine availability"
3. Update Part 2.5.4 — feed taxonomy table: add `feed_elements` row, update `feed_items` description
4. Update Feature 3 section (Part 2, line 79) — `internal_status` → `current_state (artist-defined)`, mention pipeline templates define valid states
5. Update state diagram annotation (line 86) — strengthen note that states are artist-defined per pipeline template, not system constants
6. Update Part 2.5.7 — note that `commission_status` is also a tag, not a column
7. Update Feed Renderer table (Part 4.3) — clarify feed items have elements

### Feature 1 (01-atproto-auth/README.md)

8. Simplify Enables section — only list Feature 2. Add note: "All other features depend on auth transitively through Feature 2."

### Feature 2 (02-identity-profile/README.md)

9. Fix `orgs` table schema to match codebase direction: remove `owner_id`/`created_by`, add `bio`, `avatar_url`, `updated_at`. Ownership via `org_members`.
10. Fix `org_members` table: add `id`, `is_owner`, `permissions (BIGINT)`, `updated_at`. Change role to `TEXT` (not enum). Document `default_roles` table.
11. Add `feed_subscriptions` table definition (moved from Feature 6)
12. Update `feed_items` table — remove `item_type` and `content_json`, add reference to `feed_elements`
13. Add `feed_elements` table definition
14. Remove `commission_status` from section 2.1 (line 20)
15. Remove `onboarding_completed_at` from the users table note (line 33) — wait, owner said keep it. Keep it but add note: "This is a platform lifecycle field, not a feature flag."
16. Document permissions bitfield in section 2.1 or 2.3 (currently only in code)
17. Add `default_roles` table and explain role → permission mapping
18. Update `feeds` table with `description`, `updated_at`, `deleted_at`

### Feature 3 (03-commission-engine/README.md)

19. Replace `internal_state ENUM` with `current_state TEXT` validated against pipeline template
20. Pipeline templates define: valid states, transitions, terminal states
21. System only distinguishes "active" vs "terminal" for payment/deadline/dispute purposes
22. Remove any feed field from Commission entity — feeds attached via `entity_feeds`
23. Update event types — `StateChanged` carries the new state name, not an enum value
24. Commission shell: `id, pipeline_template_id, current_state, created_at, completed_at, deleted_at`
25. Update the Cancelled note — cancellation is a terminal state defined in the template, not a special case

### Feature 5 (05-omnichannel-comms/README.md)

26. Chat feed is NOT auto-created by the system. It's a plugin add-on that creates its own feed. Commission only has one system feed (events). Chat is bolted on by a chat plugin.
27. Remove "auto-create a chat feed" language
28. Reframe: "The chat add-on (a built-in plugin) creates and manages a chat feed attached to the commission via `entity_feeds`"

### Feature 6 (06-plugin-ecosystem/README.md)

29. Remove `feed_subscriptions` table definition (now in Feature 2)
30. Reference Feature 2 for the canonical feed subscription schema

### Feature 7 (07-community-analytics/README.md)

31. Remove `commission_status = 'open'` references — replace with "org publishes availability via tags and feed posts"

### Feature 8 (08-search-discovery/README.md)

32. Note that `commission_status` is a tag, not a column — search filters by `status:open` tag

### OVERVIEW.md

33. Remove F1→F3 and F1→F9 direct edges from Mermaid graph
34. Add note: "All features require authentication (F1) transitively through Feature 2"

### inconsistencies.md

35. Delete this file after changes are applied — it's been consumed

---

## Not Changing

- `onboarding_completed_at` on users — owner approved keeping it as platform lifecycle field
- `entity_feeds` junction table model — owner confirmed entity_type enum approach
- `is_owner` on org_members — stays as convenience flag
- Permissions bitfield implementation — stays, just needs to be documented in spec

---

**Delete this document after all changes are applied.**
