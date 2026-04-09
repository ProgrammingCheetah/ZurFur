# Design Document Inconsistencies

Findings from a manual review of all 15 design files on 2026-04-08.

## Remaining Issues

### 1. `onboarding_completed_at` on `users` table breaks atomic User principle

**Feature 2, line 33:** `users table gets onboarding_completed_at (nullable timestamp)`

This adds a feature-specific column to the User entity, which contradicts the core principle that User is atomic (identity only). Onboarding state should live elsewhere — either on the personal org or as a separate `user_onboarding` table. The design document (Part 2.5.2) explicitly states: "No bios. No roles. No feature flags."

My Solution: If we were to put it in the personal org, we would have to replicate this for every single row. That is a waste of space, even if only boolean. I think we can keep it in the User Identity, simply because it makes sense that a User's identity is tied to the platform in one way or another.

### 2. `org_members.role` enum vs onboarding roles are different sets with unclear mapping

**Feature 2, line 18:** `role (enum: owner/artist/collaborator/member)`
**Feature 2, line 30:** onboarding roles: `artist, crafter_maker, commissioner_client, coder_developer`
**Feature 2, line 36:** mapping documented but `crafter_maker` maps to `artist` role

The `crafter_maker` onboarding role maps to the `artist` org role, losing the distinction. A fursuit maker and a digital artist are both just "artist" in the system. If the platform needs to distinguish them later (e.g., for search), this information is only preserved if they also set appropriate tags. This may be intentional but could surprise implementers.

My Solution: These do not feel like they should be in an Enum at all. They should be separate, since we can have an infinite amount of different roles. We should, however, have some starting, default roles that we can allow users to choose from. We can have a default_role table that includes the ones mentioned in there. This allows us to react to them as triggers, rather than states.

### 3. `commission_status` location ambiguity

**Feature 2, line 20:** "commission_status (open/closed/waitlist) lives on the org"
**Feature 2, line 49:** "commissions (only if org has artist/crafter role) — commission openings and status"
**Design doc, line 293:** "Commission availability — an org-level setting"

Commission status is described both as a field on the org AND as feed items in the commissions feed. These serve different purposes (current state vs history), but the spec doesn't clearly state which is the source of truth. Is `commission_status` a column on the `orgs` table, a tag on the org, or derived from the latest feed item?

My Solution: We shouldn't have a commission status in there. If anything, it should be in the bio. Since everybody can have a Commission Feed, there is a timeline. The bio acts as a state that the user can read, but the system doesn't care about. If we wanted more, we can add "tags" to anybody. The same we would add to characters. Even better, entities can have tags. That means feeds, orgs (and, therefore, users through personal orgs) and characters.

### 4. `orgs` table schema doesn't match codebase

**Feature 2, line 17:** `orgs table: id, owner_id, slug, display_name, bio, avatar_url, is_personal, created_at, deleted_at`
**Actual codebase (from implementation):** `organizations table: id, slug, display_name (nullable VARCHAR), is_personal, created_by, created_at, updated_at, deleted_at`

Differences: Feature 2 spec says `owner_id` but codebase uses `created_by`. Feature 2 spec includes `bio` and `avatar_url` directly on the orgs table, but the codebase has these on the separate `organization_profiles` table. Feature 2 spec is missing `updated_at`. The spec and code need to be reconciled — either the spec should match the implemented schema, or the code should be updated.

My Solution: Let's remove both. Members are depicted by using the `org_members` using roles. An owner will always have all of their permissions and should have the role "Owner" in the org_members table. Eventually, organizations may change owners. The bio and avatar_url should be kept in the organization, since it is part of it's domain. Let's add the `updated_at` in it too. The code should be updated.

### 5. `org_members` table schema doesn't match codebase

**Feature 2, line 18:** `org_members table: org_id, user_id, role (enum), title, joined_at`
**Actual codebase:** `organization_members table: id, org_id, user_id, role (TEXT), title, is_owner, permissions (BIGINT), joined_at, updated_at`

The spec is missing: `id` (primary key), `is_owner` (boolean), `permissions` (bitfield), `updated_at`. The spec uses an enum for role, but the codebase uses free-text TEXT. These are significant differences an implementer would hit immediately.

My Solution: Let's change this to a role-based system. Roles are quick ways of setting permissions. So, the table would contain a title and a role (Which can only be owner, admin, mod and member). It has to be an Enum because changing this in the codebase HAS to mean something. The title can be set by themselves or anybody with more powers than them (or with the permission to do so). We SHOULD always have an id. We can keep `is_owner`, `permissions` and `updated_at`. They seem good things to have. The role can 

### 6. Permissions bitfield not mentioned in Feature 2 spec

The codebase implements a `Permissions` bitfield (BIGINT) on `organization_members` with constants: MANAGE_PROFILE, MANAGE_MEMBERS, MANAGE_COMMISSIONS, CHAT, MANAGE_TOS, MANAGE_PAYMENTS. Feature 2's spec doesn't mention this at all — it only describes role-based access. The design document (Part 2.5) mentions permissions but Feature 2's implementation approach section does not describe the bitfield.

My Solution: Roles should be easy ways to modify the Permissions when adding someone to an organization and nothing else for them. The only other thing we should consider them with is that Owners are above admins, admins are above mods, mods are above members.

### 7. Feed subscription table not defined

**Feature 6, line 22:** `feed_subscriptions table: feed_id, subscriber_org_id, permissions, granted_at, granted_by_user_id`

This is defined in Feature 6 but never in Feature 2 (where feeds are introduced). Since feeds are foundational infrastructure (Feature 2 Phase 2), the `feed_subscriptions` table should be defined in Feature 2, not Feature 6. Feature 2 mentions "Following an org = subscribing to its feeds" but doesn't define the subscription table.

My Solution: Move feeds to Feature 2. They are a main Domain topic, so we should treat them as first class citizens. We did define them. Everything is an entity- orgs, characters, etc. (Users are not, because their profiles are managed through orgs.) Commission cards have feeds inside of them as well. 

### 8. `entity_feeds` ownership model inconsistency

**Feature 2, line 44:** `entity_feeds table: feed_id, entity_type, entity_id`
**Memory/design discussions:** Described as "any entity that can have a feed gets a feed" with `owner_type + owner_id`

The `entity_feeds` table in Feature 2 uses `entity_type + entity_id`, which is the relationship from entity → feed. But the feed itself has no `owner` — there's no way to query "who owns this feed?" without joining through `entity_feeds`. The design discussions mentioned feeds being standalone with `owner_type + owner_id` on the feed itself, but Feature 2's schema puts the relationship on a junction table instead. Both work, but the architecture discussions and the spec describe slightly different models. 

My Solution: We removed owner_type and owner_id. We went for entities. Anything can be an entity in our design, but, again, changing them should mean something. I think an entity_type should be part of an enum. As for the entity_id, the Domain would look for things based on the type. If it is an org, it would look on a table. A user? Another table. A character? You guessed it, another table. This also applies for the commission's feed. 

### 9. Design document state diagram still shows non-canonical states

**Design doc, lines 88-119:** The Mermaid stateDiagram includes states like `Inbox`, `Reviewing`, `Declined`, `Active_Workflow`, `Awaiting_Approval`, `In_Progress`.

There IS an annotation above it (line 86) explaining these are board projection concepts. However, the diagram itself doesn't visually distinguish projection states from internal states. The four canonical states (`Blocked`, `InProgress`, `AwaitingInput`, `Completed`) are not highlighted or annotated within the diagram. A reader could still be confused about which states are real.

My Solution: We kind of changed the way that stuff works regarding commissions and all. Commissions have an internal state (It only knows about it's own state, which can be whatever they want, including nothing), and an external state that exists only in the context of the view. This view, such as Kanban and all, knows about the Commission's state in their own place.

### 10. Feature 5 chat feed vs commission event feed — two feeds per commission

**Feature 5, line 16:** "auto-create a `chat` feed attached via `entity_feeds`... in addition to the commission's event feed"
**Feature 3, line 28:** "Commission gets a system feed auto-created on commission creation"

So a commission has TWO feeds: an event feed and a chat feed. But `entity_feeds` links entity → feed, meaning you'd have two rows with `entity_type = 'commission'` and the same `entity_id` but different `feed_id`. How does the system distinguish them? By `feed.slug`? This works but the spec should explicitly state that entities can have multiple named feeds, distinguished by slug.

My Solution: We are not going to overcomplicate that kind of stuff through the program. This is all going to be managed through plugins or the like. Feeds should be kept as pure as possible.

### 11. Feature 1 "Enables" section claims too many direct dependencies

**Feature 1, lines 68-72:** Claims it enables Features 2, 3, 8, and 9 directly.

Features 3, 8, and 9 all also depend on Feature 2 (orgs, feeds). The OVERVIEW graph now has direct edges F1→F3 and F1→F9, but these are only needed for "authenticated users" — the substantial dependency is through Feature 2. This isn't wrong, but it makes the dependency graph busier than necessary. A note like "all features require auth (F1) transitively" would be cleaner than N direct edges.

My Solution: Can you fix this? We can talk about it if you want

### 12. No canonical definition of `feeds` table columns across features

**Feature 2, line 43:** `feeds table: id, slug, display_name, feed_type (enum: system/custom), created_at`
**Feature 6, line 22:** references `feed_subscriptions` but not the feeds table itself

The `feeds` table is missing several potentially needed columns: `description`, `created_by` (who created a custom feed?), and any relationship back to its owning entity. If a custom feed is created, who owns it? The `entity_feeds` junction links entities to feeds, but there's no `created_by` on the feed itself.

My Solution: Pretty much, a feed should only know what it is about, when it was created, updated, or deleted (Everything in our system should be soft-deletable on domain). I think it should have a name, a description, tags, type (System/Custom). Permissions with feeds are a bit different. I think a feed should be able to be deleted by whoever "owns" it or has permissions. For example, an admin of an org can delete a feed belonging to the org if they got permissions to do so. What do you think?

---

## Non-Issues (Verified Correct)

- `feed_post` is fully replaced with `feed_item` everywhere
- `awaiting_payment` is fully replaced with `awaiting_input`
- `Stripe/PayPal` is replaced with `Stripe` only
- No remaining `is_artist`, `artist_profiles`, `artist_id` in active contexts
- No remaining `character_gallery_items`, `card_messages`, `chat_bridges` tables
- All features reference correct dependency directions
- Two-tier data split is noted where relevant
- Five root aggregates consistently referenced

---

**Delete this document after correct and consumed.**
