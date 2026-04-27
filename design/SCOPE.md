# Scope & Redesign

> **Created 2026-04-26** — working document. Companion to `NEW_DESIGN.md` and `design/infrastructure/layers/DESIGN.md`. Where this conflicts with `design_document.md` or per-feature READMEs, this doc takes precedence until reconciled.

## Why this doc exists

Three architectural shifts converged in April 2026:

1. **"Everything is a Document" → "Everything can be Referenced."** The Documents-as-substrate abstraction is retired. The shared property across User, Org, Character, Commission, Post, Tag, Feed, etc. is **addressability** — each thing has typed identity that other things can point at. There is no shared substrate type.
2. **Layers refactor in flight** (`design/infrastructure/layers/DESIGN.md`) — typed IDs, a new `policy` crate, command/handler pattern. Every feature lands inside this structure.
3. **Trait hierarchies dropped** — `Entity` / `Taggable` / `FeedOwnable` removed. Capability is expressed by what fields/relations a struct has, not which traits it implements.

This doc cuts each feature into **NOW** (MVP critical path) vs. **ROADMAP** (deferred), and surfaces the redesign questions whose answers shape multiple features. NOW/ROADMAP buckets are seeded from each README's existing phasing and are **starting points to revise, not commitments**.

**Critical path orientation:** Auth (1) → Org (2) → Tags (3) → Feeds (2+) → Characters (2.3) → TOS (11) → Commission (4) → Payments (5). Anything off this path is ROADMAP unless explicitly decided otherwise.

Markers used below:
- `↻` — item that specifically needs re-thinking under the new framing (Document removal, references, feeds-as-projections, etc.)
- `>` — blank line for inline answer

---

## Cross-Cutting Redesign Decisions

These shape multiple features. Decide here, then per-feature sections inherit.

### A. What is a "reference," in code?

Options (not exclusive — could be layered):

- **(a) Typed ID only** — `UserId`, `OrgId`, `CommissionId`, … Already in `domain/src/ids.rs`. Internal-only; serialization formats decided per-edge.
- **(b) URI string** — `at://did:plc:.../app.zurfur.character/abc` for federated, `zurfur://commission/uuid` for private. One grammar, two namespaces.
- **(c) Tagged enum** — `Ref::User(UserId)`, `Ref::Org(OrgId)`, … Polymorphic handle when a column or message can point at multiple kinds.
- **(d) Layered: (a) internally, (b) at the edge** — typed IDs in code, URIs only when serializing to atproto / API responses / event payloads.

> Depends on whether something is public or private. In the case of public, we would use URI Strings and so. Although, we need to consider how to wrap them on the outgoing. They could be considered blobs or something similar. 

### B. Does a reference carry a storage/federation hint?

i.e. should a reference encode whether the referent lives on a PDS vs. Zurfur's DB, or is that resolved separately?

- **Embedded in the reference shape** — `at://...` vs. `zurfur://...` makes federation visible at every callsite.
- **Separated** — references are uniform; a resolver looks up where the referent lives.
- **Doesn't matter** — every callsite already knows the kind, and the kind implies the storage.

> They should, yes. This makes it easier to understand. 

### C. Public vs. private split — single rule

NEW_DESIGN.md proposed: "nothing goes on AT Proto unless the user would be comfortable posting it on their public Bluesky timeline." Confirm, refine, or reject as the platform-wide rule.

> We have two scopes: Private and Public. Commissions are ALL private until they produce something, in which case the product can be posted. Out of scope right now. 

### D. Lexicons — only where they buy something

With Documents retired, Lexicons are only relevant for things that actually publish to atproto. Candidate Lexicon-defined records (per current docs): public character profile, public post / commission-slot listing, ToS snapshot, org metrics. Everything else is a Zurfur-internal type with no Lexicon.

Confirm the candidate list, add/remove:

> This is a good point, but Documents were being used as another semantic item in the project. We need to make another document called VISIBILITY.md to decide whether stuff is public or private. Public data will be Lexicon-defined (Documents) while private ones wouldn't. Documents were just composable, React-Esque pieces of data. We will completely redefine our objects.

### E. Feeds as saved queries — implications

NEW_DESIGN.md decided feeds are projections over an event stream, not containers. Several features were designed assuming feed-as-container (commission feed, chat feed, notification feed, gallery feed, bio feed). Each needs revisiting:

- **Commission "feed"** (Feature 4) — was the source of truth for events. ↻ Is it still a feed, or is it just an append-only event log addressed by the commission's reference?
- **Chat "feed"** (Feature 6) — was "messages are feed items." ↻ Same question.
- **Notification "feed"** (Feature 10) — was a per-user feed. ↻ Saved query over events targeting that user?
- **Gallery / bio "feed"** (Feature 2) — public projection. ↻ Saved query over the org's public posts/characters?

How do we want to model these? Open question:

> We are going to redefine feeds. We need to create a Glosary beforehand.

### F. The event stream — one log or many?

If feeds are projections, where does the underlying event stream live?

- **One per aggregate** — commission events, org events, user events, etc. Feeds project across.
- **One unified stream** — every create/edit/supersede appends here. Feeds filter by predicate.
- **Two streams: federated + private** — public events also go to the user's PDS firehose; private events go to a Zurfur-internal log. Per-NEW_DESIGN.md, two physical streams, one logical interface.

> 

### G. Tags — per-target mechanism (carry-over from NEW_DESIGN.md)

Open question still unresolved in NEW_DESIGN.md: tags on **users** (DIDs, no record to attach a `tags` field to) — atproto labels (Zurfur as labeler) or local-only?

> 

### H. Plugins as "orgs subscribing to feeds" — does this still work?

Feature 7's whole model is plugin orgs subscribing to feeds. With feeds = saved queries, "subscribing to a query" means "be notified when the query's result set changes." That's a different mechanic than "subscribing to a container that gets new items appended." Revisit:

> 

### I. Add-on slots on commissions

Feature 4's add-on mechanism was "feed subscriptions OR iframes." If chat-feed is a query, the chat add-on slot's contract changes. Revisit when 4 is on the table.

> 

---

## Per-Feature Scope

Each section: **status**, one-liner, **NOW** items, **ROADMAP** items, redesign open questions where they exist. NOW/ROADMAP are seeded from the README's existing phasing and are **drafts**.

---

### 1. AT Protocol Auth & Bluesky Integration

**Status:** Phase 1 done (OAuth, JWT, refresh tokens). Phases 2–3 not started.
**One-liner:** Bluesky OAuth is the only auth path; users log in via DID; platform issues its own JWTs.

**NOW:**
- OAuth (PKCE, DPoP, token refresh) — done
- AuthService + JWT — done
- Refresh token rotation — done

**ROADMAP:**
- AT Protocol token auto-refresh
- Rate limiting on auth endpoints
- DM integration (WebSocket/SSE)
- Cross-posting to Bluesky
- Social graph import

**Open questions:**
- Anything to revisit under the new framing? Probably not — auth is mostly orchestration.
  > 

---

### 2. Identity & Profile Engine

**Status:** Phase 1 (orgs, members, personal orgs) done. Phase 2 (feeds, onboarding) done. Phase 3 (characters, profile customization) not started.
Additional docs: `PHASE3_DESIGN.md`, `PHASE3_API.md`, `PHASE3_QUESTIONS.md`.
**One-liner:** Org-centric identity. User is atomic; everything public lives on orgs. Every user gets a personal org. Characters are org-scoped.

**NOW:**
- Org model + role-based membership — done
- Personal org auto-creation — done
- Onboarding (role selection) — done
- Default feeds per org (updates, gallery, [commissions]) — done ↻ (revisit under feeds-as-queries)
- Characters + reference sheets ↻ (Document-shaped before; now: typed entity, public-by-intent → atproto via Lexicon)
- Content rating (SFW/NSFW)

**ROADMAP:**
- Profile customization (CSS sanitization, layout JSON)
- Profile versioning, CSS change tracking
- Character ownership transfer, collaborative characters
- Profile analytics (folds into Feature 8)

**Open questions:**
- Bio-as-feed (Feature 3 decision) — does it survive feeds-as-saved-queries? If a feed is a query, "the bio feed" is a query over the org's bio-edits. Cleaner than a container but the storage shape changes. ↻
  > 
- Character: Lexicon shape and what fields are public vs. private?
  > 

---

### 3. Tag Taxonomy & Attribution

**Status:** Phase 1 done (typed tags, entity_tag junction, org tag auto-creation, immutability rules).
**One-liner:** Cross-cutting metadata. Typed tags (org / character / metadata / general). Entity-backed tags = identity markers (immutable).

**NOW:**
- Typed tags + entity_tag junction — done
- Entity-backed identity tags — done
- Org tag auto-creation — done

**ROADMAP:**
- Tag synonyms / aliases for search
- Curation / approval workflow
- Tag usage analytics, trending tags
- Tag cleanup (merge duplicates)

**Open questions:**
- Federation: which tags publish to atproto via the `tags` field on records, which use atproto labels, which stay local? See cross-cutting question G.
  > 
- Authorization on tag routes (flagged in Feature 3 retro): who can update / delete / approve? In NOW or ROADMAP?
  > 

---

### 3b. Transaction Support (UoW)

**Status:** Phase 1 (composite methods) done pending merge.
**One-liner:** Atomic multi-step persistence. Phase 1 = composite trait methods; Phase 2 = full UoW.

**NOW:**
- Composite methods on tag and feed repos — done
- Compensating-rollback removal — done

**ROADMAP:**
- Full UnitOfWork pattern with `_with(tx)` variants

**Open questions:**
- Layers refactor introduces handler-owned transactions (`pool.begin()` in handler, pass `&mut *tx` to persistence functions). Does that obviate the UoW abstraction or just rename it? ↻
  > 

---

### 4. Headless Commission Engine

**Status:** Not started. Critical path.
**One-liner:** Commissions are headless data shells. Artist-defined pipeline. Every action emits an event. Boards are projections. Add-on slots extend.

**NOW (proposed cut — heavily revisable):**
- Commission entity (typed reference) ↻ (was Document; now: row with refs to org, client, TOS version, spec)
- Artist-defined pipeline templates (basic — single template per org)
- Commission event stream (append-only) ↻ (was "commission feed"; now event log addressed by commission ref — see cross-cutting E)
- Multi-party (artist org + client user)
- TOS acceptance recorded on creation

**ROADMAP:**
- Add-on slots (server-side feed subscriptions, client-side iframes) ↻ (mechanic depends on cross-cutting H/I)
- Board projections (Kanban, list, calendar) — Kanban only for NOW?
- Event replay tooling, snapshotting
- Undo/rollback, draft commissions, archive
- Deadline analytics

**Open questions:**
- Is a commission a row with a reference-to-spec (separate spec entity) or a single row with the spec inlined? ↻
  > 
- Pipeline template: is it itself a referenceable entity (so commissions point at a template version), or just a config blob on the org?
  > 
- Boards: in NOW or ROADMAP? If NOW, only Kanban?
  > 

---

### 5. Financial & Payment Gateway

**Status:** Not started. Critical path.
**One-liner:** Zurfur is merchant of record via Stripe Connect. Platform holds funds, pays out to orgs.

**NOW (proposed cut):**
- Stripe Connect onboarding (Standard or Express)
- Single invoice per commission (no installments)
- Voluntary fee coverage at checkout
- Webhook ingest (payment success/fail)
- Audit trail (transaction log)

**ROADMAP:**
- Payouts (Stripe Transfer API)
- Multiple invoices per commission, installment plans
- Multi-currency, cryptocurrency
- Tax collection / 1099 / VAT
- Chargebacks, PCI review
- Voluntary fee distribution model (artist vs. platform split)

**Open questions:**
- Escrow-lite: NOW or ROADMAP? If MVP doesn't escrow, what does the artist-payout flow look like?
  > 
- Account type for orgs: Stripe Standard or Express in NOW?
  > 

---

### 6. Omnichannel Communications

**Status:** Not started.
**One-liner:** Chat is a first-party plugin add-on (not intrinsic to commissions). Bridges to Telegram/Discord/Matrix via plugin orgs.

**NOW (proposed cut):**
- In-platform chat tied to a commission ↻ (was chat-feed; now: append-only message stream addressed by commission ref?)
- File attachments (S3)
- WebSocket delivery + REST fallback

**ROADMAP:**
- Telegram / Discord / Matrix bridges (depend on Feature 7 plugin orgs)
- E2EE evaluation
- Edit / delete sync
- Threading, read receipts, typing indicators

**Open questions:**
- Is chat in NOW at all? If commissions ship without chat, comms is fully ROADMAP and clients/artists handle off-platform until chat lands.
  > 
- "Chat feed" → "chat stream addressed by commission ref"? ↻
  > 

---

### 7. Plugin Ecosystem

**Status:** Not started. ROADMAP-leaning.
**One-liner:** Plugins are orgs. Subscribe to feeds, react by posting feed items, or render iframes client-side.

**NOW:**
- (proposed) None. Plugin model rests on cross-cutting H — defer until that's decided.

**ROADMAP:**
- Plugin orgs + webhook event delivery ↻ (mechanic depends on feeds-as-queries)
- HMAC signing, exponential backoff
- Iframe sandbox + postMessage API
- Marketplace (directory, approval, ratings, paid plugins)
- Native analytical plugins
- Plugin versioning, SDK docs

**Open questions:**
- Confirm: plugins entirely ROADMAP for MVP?
  > 
- Plugin model rethink under feeds-as-queries — see cross-cutting H. ↻
  > 

---

### 8. Community & Analytics

**Status:** Not started. ROADMAP.
**One-liner:** Engagement + transparency. Feed subscriptions, XP, badges, strategy engine.

**NOW:** None.

**ROADMAP:**
- Feed subscriptions to org commission-status feeds ↻ (saved query over org status?)
- Gamification (XP, badges, levels)
- Strategy engine (org metrics, client risk)
- Public metrics → PDS
- Leaderboards (cosmetic)

**Open questions:**
- Confirm: fully ROADMAP?
  > 

---

### 9. Search & Discovery

**Status:** Not started.
**One-liner:** Full-text + tag-faceted search. "Open Now" view. Recommendations.

**NOW (proposed cut):**
- Org full-text search (PostgreSQL `tsvector`)
- Tag-driven faceted filtering
- "Open Now" view (filter by `status:open` tag)

**ROADMAP:**
- Recommendation engine (heuristic v1, ML v2)
- Meilisearch / Elasticsearch migration
- Geographic / timezone search
- Saved searches / alerts

**Open questions:**
- "Open Now" feed view: is this a saved query (cross-cutting E)? Likely yes, and aligns nicely.
  > 

---

### 10. Notification System

**Status:** Not started.
**One-liner:** Unified delivery. In-app feed + browser push + email digests + webhooks.

**NOW (proposed cut):**
- In-app notification feed per user ↻ (saved query over events targeting the user?)
- WebSocket real-time delivery
- Notification preferences (basic)

**ROADMAP:**
- Browser push (VAPID, service worker)
- Email digests (background job)
- Webhook notifications (legacy path; plugins use Feature 7)
- Batching / deduplication
- (Explicitly not building: SMS)

**Open questions:**
- "Notification feed" → user-scoped saved query over the event stream (cross-cutting E/F). ↻
  > 

---

### 11. Organization Terms of Service

**Status:** Not started. Critical path (gates commission acceptance).
**One-liner:** Orgs publish versioned TOS. Clients must accept current version before commissioning. Active TOS publishes to PDS for audit.

**NOW (proposed cut):**
- Structured TOS (JSON sections)
- Versioning (immutable snapshots, one active per org)
- Mandatory acknowledgment on commission creation
- PDS publishing (Lexicon: ToS snapshot)
- Diff view for returning clients

**ROADMAP:**
- TOS template library
- Legal review / canonical templates
- i18n
- Bulk TOS-change handling for in-flight commissions

**Open questions:**
- TOS as a referenceable entity: a commission references the TOS version it was created under (immutable ref). Confirm.
  > 

---

### 12. Content Moderation & Trust/Safety

**Status:** Not started.
**One-liner:** Block / mute, reporting, DMCA, automated NSFW flagging.

**NOW (proposed cut — minimum for private beta):**
- Block / mute (user → user, user → org)
- Reporting system (categories, rate-limited)
- "Untagged NSFW" detection (tag-system integration)

**ROADMAP:**
- DMCA workflow + PDS takedown coordination
- DMCA counter-notice / restoration
- Community flag thresholding / auto-escalation
- Plugin org reporting + disable

**Open questions:**
- Confirm minimum for private beta. Public launch needs more.
  > 

---

### 13. Dispute Resolution

**Status:** Not started.
**One-liner:** When commissions go wrong with money in escrow: file dispute, freeze payout, evidence, resolution.

**NOW:** None? (depends on escrow model in Feature 5; if no escrow in MVP, no formal disputes either).

**ROADMAP:**
- Dispute filing (creates add-on slot, freezes payouts)
- Evidence submission (text + files, refs to commission events)
- Auto-resolution rules
- Manual moderator review
- External mediation
- Early-warning prevention

**Open questions:**
- If MVP has no escrow, MVP has no formal disputes either. Confirm this is fully ROADMAP.
  > 

---

### 14. Platform Administration

**Status:** Not started. Required before public launch, not before private beta.
**One-liner:** Internal tooling for operators. Suspensions, audit, moderation queue, PDS admin ops.

**NOW (proposed cut — minimum for private beta):**
- User / org suspend / unsuspend (CLI or minimal endpoint)
- Basic audit log

**ROADMAP:**
- Financial audit dashboards, reconciliation, CSV export
- Moderation queue UI (reports, disputes, flags, feed items)
- Plugin org moderation
- AT Protocol admin operations (PDS takedowns, labeling)
- System health metrics (Prometheus / Grafana)

**Open questions:**
- Admin role on User (atomic) vs. an admin org? Currently the design implies admin role on user. ↻ Confirm.
  > 

---

## Process

We walk through the sections together. For each:

1. Cross-cutting questions first (A–I) — answers shape every per-feature decision.
2. Per-feature: confirm or revise NOW / ROADMAP buckets, answer open questions inline.
3. Once a section is settled, move it to a "Decided" status and the per-feature README + design docs get rewritten to match.

What changes between this doc and the per-feature READMEs after we finish: the READMEs become reflections of decisions made here, not the reverse.
