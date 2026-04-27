# Glossary

> **Created 2026-04-26** — working document. DDD-style ubiquitous-language tables.
>
> **Supersedes** `design/glossary.md` (lowercase, dated 2026-04-12) once entries are settled. The lowercase doc is now contradicted in several places and stays only as historical reference.
>
> **Companions:** `SCOPE.md` (MVP vs. roadmap), `NEW_DESIGN.md` (architectural philosophy in flux), `VISIBILITY.md` (TBD — full Public/Private rules), `design/infrastructure/layers/DESIGN.md` + `LAYER_RULES.md` (layer architecture).

## Legend

**Status markers** (rightmost column on every table):
- `✓` — confirmed by you (explicitly written or directly extracted from your Object definition).
- `?` — drafted by Claude; your confirmation needed. Open Question section flags the substantive ones.
- `↻` — open; needs a decision. See **Open Questions** at the bottom.
- `→` — deferred to roadmap.

**DDD types used:**
- **Aggregate** — root entity that manages internal consistency for a cluster; the only entry point.
- **Entity** — has identity, mutable, lives within an aggregate (or is the root).
- **Value Object** — defined by attributes; no identity; immutable.
- **Concept** — shared vocabulary that's not strictly DDD-classified.
- **External** — defined by another system (atproto), listed for shared vocab.
- **Layer / Pattern / Process** — architectural construct.

---

## Foundation: Your Object Definition

The four first-class citizens and the surrounding concepts, verbatim from your fill-in. Every table below builds on this.

> Before, everything was a `Document`. Because this was part of really early optimization, we are scratching that and just allowing the application to evolve naturally.
>
> Instead of everything being a document, we are moving towards a different type of relationship: `Everything can be referenced`. Composition is resolved this way as well.
>
> We are making basically 4 things be first-class citizens at the start: **User, Org, Commission and Character**. These are defined as the following:
>
> - **User**
>   - The Canonical user, taken from Bsky's AT Protocol OAuth and identified by a DID.
>   - Main actor of the application, different from before, where an Organization was the main actor of an application.
>   - They are the primary actor of the application.
> - **Organization**
>   - A way to group Users.
>   - Every user gets one personal, "Anchor" organization by default, although it may go unused (Not every user is an Artist).
>   - Before, organizations were the primary movers and owners of everything, but now are just an easy way to collect users into things.
>   - Commissions are owned by organizations and can be assigned users. Every member of an organization can see every commission belonging to an organization.
>   - Anchor organizations can only contain one user in the organization; "Studio" organizations are organizations that can contain many users.
> - **Commission**
>   - A Unit of Work. It is worked on by artists.
>   - Artists have administrative powers over commissions, such as creating, deleting, updating, etc.
>   - Commissions are composed of intrinsic attributes (title, created_at, etc.) and feeds. Commissions can contain N feeds.
>   - Commissions are NOT tied to Views or UI. They can be viewed by ANY user that has a membership in the commission.
> - **Character**
>   - A character is a collection of images (blobs), descriptions, etc.
>   - A character belongs to a User, not an organization. We are not using mascots or "shared characters" in our use cases.
>
> Alongside these:
>
> - **Visibility** — Two visibility models: Private and Public. Private = lives with the application, in the DB or our own persistence. Public = lives in the AT Protocol and is federalized; manifested as Records or Documents (same thing — joint definition pending).
> - **Blobs** — Opaque bytes representing media. Content-addressed by CIDs; content hashes strongly advised. Can be referenced. Smallest unit of content; no authorship, identity, or behavior.

---

## Notable shifts from the prior framing

Quick orientation for anyone reading this fresh:

- **Primary actor** flipped: was Organization, now User.
- **Personal Org** evolved → **Anchor Org**; not a rename — Anchor is a semantically distinct type from Studio (single user, auto-created vs. multi-user, intentionally created).
- **Character** ownership flipped: was Org-scoped, now User-scoped. No mascots / shared characters.
- **"Everything is a Document"** retired → **"Everything can be Referenced"** (composition included). The semantic primitive is now **Block**; the public wire shape on atproto is **Record** (atproto's own term). "Document" is dropped from the Zurfur ubiquitous language.
- **Tag** flipped from first-class root aggregate → deferred to roadmap (introduced non-needed things).
- **Feed** renamed → **Chain** (a line of blocks). Owned by a parent entity (e.g. a Commission contains N Chains). The "universal content container" framing is dead.
- **Block** named — the recursive, composable, blob-mounting primitive a Chain contains. Themed: **Blob** (raw bytes) → **Block** (composable unit on top of blobs) → **Chain** (ordered sequence of blocks). Same primitive across public (serialized as atproto Records) and private (JSON in DB) — visibility is *where-stored*, not *what-shape*.
- **References** are URI strings with AT-URI grammar. Public uses atproto-native URIs; private uses a `zurfur.private` namespace. Storage location is visible at every callsite.
- **Roles** renamed: old `Owner / Admin / Mod / Member` → `Owner / Admin / Manager / Member` (Mod → Manager). **Artist** is a separate Role that applies at both Org and Commission levels. **Org Membership** grants visibility; **Commission Assignment** grants admin powers.

---

## 1. Aggregates & Entities

The four first-class citizens and their internal entities.

| Term | Type | Definition | Status |
|------|------|-----------|--------|
| **User** | Aggregate | Canonical user from atproto OAuth; primary actor of the application. Identity: `DID` (atproto) + `UserId` (internal newtype). Owns: an Anchor Org (auto-created), Characters. | ✓ |
| **Organization** | Aggregate | A grouping mechanism for Users. Owns Commissions; contains Members (Users). Two variants: Anchor (one user) and Studio (many users). Identity: `OrganizationId`. | ✓ |
| **Commission** | Aggregate | A Unit of Work, worked on by artists. Composed of intrinsic attributes (title, `created_at`, …) plus N feeds. Decoupled from views/UI. Owned by an Organization. Identity: `CommissionId`. | ✓ |
| **Character** | Aggregate | An OC: a collection of blobs (images), descriptions, etc. Owned by a User (not an Org). Identity: `CharacterId`. | ✓ |
| **Anchor Org** | Entity (Org type) | Single-user Organization. Evolution of the old "Personal Org" — more semantically correct and versatile. Auto-created with each User; may go unused. **Distinct type** from Studio Org. | ✓ |
| **Studio Org** | Entity (Org type) | Multi-user Organization. Always created intentionally (after the fact); never auto-created. **Distinct type** from Anchor Org. | ✓ |

---

## 2. Value Objects

| Term | Type | Definition | Status |
|------|------|-----------|--------|
| **Reference** | VO | Addressable handle to any thing. **Always URI strings using AT-URI grammar.** Public references use atproto-native URIs (`at://did:plc:.../com.zurfur.character/abc`); private references use a Zurfur namespace (`at://zurfur.private/commission/uuid`). Namespace makes storage/federation visible at every callsite. Replaces "everything is a Document." | ✓ |
| **Blob** | VO | Opaque bytes representing media. Content-addressed by CID; content hashes advised. Can be referenced. Smallest unit of content; no authorship, identity, or behavior. | ✓ |
| **DID** | VO (external) | atproto stable identifier for a User (`did:plc:...`). | ✓ |
| **Handle** | VO (external) | atproto human-readable alias for a DID (`@zuri.bsky.social`). Mutable. | ✓ |
| **Newtype IDs** (`UserId`, `OrganizationId`, `CommissionId`, `CharacterId`, …) | VO | UUID wrappers per entity, defined in `domain/src/ids.rs`. Prevent passing the wrong ID kind at compile time. | ✓ |

---

## 3. Visibility & Federation

| Term | Type | Definition | Status |
|------|------|-----------|--------|
| **Visibility** | Concept | Binary attribute on data: `Private` or `Public`. Drives storage location, federation behavior, and reference shape. Full rules go in `VISIBILITY.md` (TBD). | ✓ scope |
| **Private** | Visibility value | Lives in Zurfur's DB / persistence. Never federates. | ✓ |
| **Public** | Visibility value | Lives in atproto (the user's PDS) and federates. Manifested as Records (Lexicon-defined). | ✓ |
| **Record** | Public-data shape | The serialized form of a public **Block** on atproto: Lexicon-defined record stored on a PDS. Defers to atproto's own term for the wire shape. | ✓ |
| **Document** | (retired) | Retired in favor of **Record** (Q5). Dropped from the Zurfur ubiquitous language; kept here as a tombstone for searchers. | → |
| **Lexicon** | External | atproto's JSON Schema record-type definition system, referenced by `$type`. Used only for public Zurfur data. | ✓ |
| **AT Protocol** | External | Decentralized social protocol; Bluesky is the reference app. Source of OAuth identity, public storage, and federation. | ✓ |
| **PDS** | External | Personal Data Server — user-controlled atproto host; source of truth for public data. Zurfur indexes it locally for fast queries. | ✓ |
| **Firehose** | External | atproto's append-only public event stream. | ✓ |
| **Output / Product** | Concept | The publishable artifact a Commission produces. Lives in the Commission's "output" Chain. Typically a Blob wrapped in a Block (one or more Blocks each containing one output Blob). Contained by the Commission, not a separate Aggregate. | ✓ |

---

## 4. Concepts

| Term | Type | Definition | Status |
|------|------|-----------|--------|
| **Composition** | Concept | **Hybrid — follows atproto's pattern.** Inline structure for a Block's own content (text, facets, type, timestamps). References for sub-pieces — Blob CID for media, AT-URI + CID for nested/quoted Blocks. Rich text uses **byte-range annotations** (facets), not inline markup. Mirrors `app.bsky.feed.post`'s embed-union approach. See Q7 for the full pattern + atproto sources. | ✓ |
| **First-class citizen** | Concept (informal) | The four entities present on day one: User, Org, Commission, Character. Informal alias for **Aggregate** (per Q9); both terms coexist in our semantics. | ✓ |

---

## 5. Roles & Membership

| Term | Type | Definition | Status |
|------|------|-----------|--------|
| **Org Membership** | Relation | User ↔ Organization. Grants **view-only** visibility into all Commissions owned by the org. Administrative powers come from per-Commission **Assignment**, not from Org Membership alone. Carries a Role + a Title. | ✓ |
| **Commission Assignment** (formerly "Commission Membership") | Relation | User ↔ Commission. The User is **assigned** to the commission, granting administrative powers (move it forward, update state, request payment, etc.). Distinct from Org Membership: visibility comes from Org Membership; admin powers come from Assignment. | ✓ |
| **Role** | VO | Hierarchy: **Owner > Admin > Manager > Member**. (Rename: old "Mod" → "Manager.") Administrative position within an Org. **Artist** is a separate Role kind that applies at both Org and Commission levels. | ✓ |
| **Title** | VO | Cosmetic display string ("Lead Illustrator"). Can be attached to any Role. Per-membership. | ✓ |
| **Artist** | Role | Exists at **both** Org level (an Org Member with the Artist role) and Commission level (the User assigned to a specific commission as the Artist). Can create / delete / update Commissions in their scope; the Commission-assigned Artist moves the commission forward, requests payment, etc. | ✓ |

---

## 6. Architecture & Process (settled per layers refactor)

| Term | Type | Definition | Status |
|------|------|-----------|--------|
| **Domain** | Layer | Owns entity structs, repository traits, structural errors, newtype IDs. No I/O. | ✓ |
| **Policy** | Layer | Pure business rules as free functions; workflow / contextual errors. No I/O, no SQL, no HTTP. | ✓ |
| **Application** | Layer | Thin orchestrator. Builds + dispatches commands; runs guards / policy / persist / react. | ✓ |
| **Persistence** | Layer | SQLx implementations of domain traits. Executor-generic free functions are default; trait methods are thin wrappers; action methods compose them in transactions. | ✓ |
| **API** | Layer | HTTP translation. Parses request → builds Command → dispatches to application handler → maps result. | ✓ |
| **Repository** | Pattern | Domain trait describing persistence contract per entity. Implemented in persistence; consumed by application via `&D where D: Trait`. | ✓ |
| **Command** | Pattern | Struct carrying user intent into application. Built by API; consumed by handler. Has in-struct + out-struct. | ✓ |
| **Handler** | Pattern | Free function in application taking `&D` and a Command; runs guard / policy / persist / react. | ✓ |
| **Critical Path** | Process | Minimum sequence from "user signs up" to "artist gets paid": **Auth → Org → Feeds → Characters → TOS → Commission → Payments**. (Tags removed — see Deferred.) | ? updated |
| **NOW vs. ROADMAP** | Process | Two scope buckets used in `SCOPE.md`. NOW = MVP critical path; ROADMAP = deferred. | ✓ |

---

## 7. Open Primitives (still being shaped)

| Term | Type (working) | Current understanding | Status |
|------|---------------|----------------------|--------|
| **Chain** (formerly "Feed") | Entity | An ordered sequence of Blocks. Owned by a parent entity (e.g. a Commission contains N Chains). Replaces the old "Feed" term in the Zurfur ubiquitous language; "feed" survives only as informal English. | ✓ |
| **Event / Event Stream** | TBD | Append-only stream of facts (per `NEW_DESIGN.md`). Where it lives — per-aggregate, unified, or two-physical-one-logical (private + federated) — open per `SCOPE.md` Q F. | ↻ |
| **Aggregate** (the DDD term) | DDD term | Kept. Coexists with **First-class citizen** as an informal alias (per Q9). The "no FK between aggregates" rule from `RULES.md` survives. | ✓ |
| **Block** | Entity (composition primitive) | The recursive, composable unit a Chain contains. References other Blocks; sits on top of Blobs (its leaves); carries typed sub-shapes (inline text, image, embed, …). **Same primitive across visibilities** — visibility is *where-it-is-stored*, not *what-shape-it-is*. Public Block serializes to a Record on atproto (Lexicon-defined). Private Block stays as JSON in Zurfur's DB. Theme: **Blob** → **Block** → **Chain**. | ✓ |

---

## 8. Deferred to Roadmap

Items previously treated as in-scope, now pushed.

### 8a. Tags (entire system)

> **Verbatim from your decision:** "Tags used to be first class. We are deferring them until way down the line. They introduced non-needed things."

The "non-needed things" framing means more than "later" — when Tags come back, they likely come back differently.

| Term | What it was | Status |
|------|------------|--------|
| **Tag** | Typed metadata marker (categories: organization, character, metadata, general). Was a root aggregate. | → |
| **Entity-Backed Tag (Identity Tag)** | Auto-created tag for orgs/characters; immutable; used for attribution. | → |
| **`entity_tag` junction** | Polymorphic "any entity → Tag" junction. | → |
| **TagService, TagRepository** | Application + persistence layers for tags. | → |

### 8b. Other deferred concepts

| Term | Reason | Status |
|------|--------|--------|
| **Plugin / Plugin Org** | Whole Feature 7. Subscription mechanic was a feed-as-container thing; defer entirely. | → |
| **Subscription** (feed subscription) | Subscribing to a "container" doesn't translate cleanly under the new framing. | → |
| **Notification (system)** | Whole Feature 10. Reconsider once Feed is redefined. | → |
| **Add-on Slot** | Old Feature 4 mechanism. Probably subsumed by "Commissions contain N feeds." | ↻ defer or merge |
| **Spec / Commission Spec** | Probably folded into Commission's intrinsic attributes or one of its feeds. | ↻ defer or merge |
| **Pipeline / Pipeline Template** | Old "headless commission engine" workflow concept. | ↻ defer or rethink |
| **TOS / ToS Snapshot** | On critical path per `ROADMAP.md` (gates Commission acceptance). Definition pending — promote to Open Primitives if it stays in NOW. | ↻ |
| **Board / Projection** | "Commissions are NOT tied to Views or UI" — projections exist but separated from Commission. | ↻ defer or rethink |

---

## Open Questions

The decisions waiting on you. Resolved entries get promoted into the tables above.

### Q1. Anchor / Studio Org — structural or stateful?

Are Anchor Org and Studio Org separate types, or two states of one `Organization` struct (e.g. discriminated by a flag, or just by member count)?

> Anchor Orgs is the evolution of personal orgs. They are more semantically correct and versatile, while Studios are always created after the fact. They are two types.

### Q2. Org Membership vs. Commission Membership

Your Object answer mentions both. Are these:
- (a) The same relation — org membership transitively grants commission visibility for all org-owned commissions?
- (b) Two separate relations — org membership grants visibility to org-owned commissions, plus invited-client commission membership grants visibility individually?
- (c) Something else?

> Anybody that is part of an org can see all the commissions the org contains. However, not being assigned to a commission means that no administrative powers have been given to them. Non-assigned org members are view-only to commissions like that.

### Q3. Role survival under the new Org framing

Old Org had Owner / Admin / Mod / Member. New Org is "a way to group Users." Do all four roles survive, do they collapse to fewer, or does Role go away in favor of permissions per relation?

> Owner > Admin > Manager > Member

### Q4. "Artist" — what kind of thing?

You wrote: "Artists have administrative powers over commissions." Is Artist:
- (a) A Role on an Org Membership?
- (b) A Title (cosmetic)?
- (c) A permission flag derived from membership?
- (d) Just a name for "Org member who is also assigned to a Commission"?

> It is a role in both Orgs and Commission. Titles can be assigned to roles in general, as cosmetics. An artist can create, delete, and update commissions. In the case of a commission, an Artist is assigned to a commission. They are the ones that move the commission along, ask for payment, etc. 

### Q5. Document vs. Record — canonical name

Your Visibility definition treats them as the same: "Records or Documents (Same thing)." Pick one as canonical; the other becomes alias or is dropped.

> ✓ **Answered: Record is canonical; Document is retired.** Theme stays tight to Blob/Block/Chain — don't extend past composition primitives. "Record" defers to atproto's own term for the wire shape; "Document" was the conflated name that started this thread.

### Q6. Reference shape for private things

Public references are URI strings (atproto-shaped). Private references — what?
- (a) `at://`-style URI in a Zurfur namespace (`at://zurfur.private/commission/uuid`)
- (b) Typed ID only (already exists as `CommissionId(Uuid)` etc.) — never serialized as a URI.
- (c) Opaque string (`zurfur://commission/uuid`).

> A. It is the most loyal to our way of doing things.

### Q7. Composition — concrete shape

"Composition is resolved through references." Concretely:
- (a) An entity stores composed children as a list of references (e.g. Commission has `feeds: Vec<FeedRef>`).
- (b) Composition is implicit via separate junction / relation tables.
- (c) Both — depending on whether the composition crosses the public/private boundary.

> So, this has to do a little bit with how records are saved. A record can have text, then embed media, and so on. This means that there needs to be some reconciliation. Please research this in the web.

> ✓ **Research (atproto pattern, applied to Block):** Composition is **hybrid — both inline and reference** with a clear split:
>
> 1. **Inline** — the Block's own structured content. In `app.bsky.feed.post`: `text` (≤3000 chars), `facets` (rich-text annotations), `langs`, `tags`, `labels`, `createdAt`, `$type`, `embed` (a typed union — see below). Small, cohesive fields stay inline.
> 2. **References** — anything large, reusable, or cross-record:
>    - **Blobs** (media): ref shape `{$type, ref: CID, mimeType, size}`. The Blob is uploaded separately and referenced by CID.
>    - **Other records** (quote, reply): `com.atproto.repo.strongRef` = `{uri, cid}`. URI gives location; CID pins the exact revision.
> 3. **Embed unions** — the `embed` field is a `$type`-discriminated open union: `images`, `video`, `external` (link + preview), `record` (strongRef to another record), `recordWithMedia` (media + record ref). Open union = unknown `$type`s gracefully ignored.
> 4. **Rich text via facets, not markup** — instead of HTML/Markdown inside `text`, atproto uses **byte-range annotations**: `{index: {byteStart, byteEnd}, features: [{$type, ...}]}`. Feature kinds: `mention` (DID), `link` (URL), `tag` (string). Unknown features fall through to plain text — graceful degradation.
>
> **Mapping to Zurfur Block:**
> - Block's own data: inline (`text`, `facets`, `$type`, `createdAt`, …).
> - Sub-pieces: References — Blob CID for media; AT-URI + CID for nested Blocks.
> - Rich text: facets-style byte-range annotations (not embedded markup).
> - Public Blocks ride atproto's existing Lexicon shapes with near-zero translation; private Blocks use the same JSON structure with `at://zurfur.private/...` URIs in reference fields.
>
> **Answer:** essentially **(c) Both** — with the specific inline/reference split mirroring atproto. Composition row updated.
>
> Sources: [post.json lexicon](https://github.com/bluesky-social/atproto/blob/main/lexicons/app/bsky/feed/post.json) · [AT Protocol data model](https://atproto.com/specs/data-model) · [facet.json lexicon](https://github.com/bluesky-social/atproto/blob/main/lexicons/app/bsky/richtext/facet.json) · [Why RichText facets — Paul Frazee](https://www.pfrazee.com/blog/why-facets)

### Q8. Feed redefinition

Your Object answer says Commissions contain N feeds. What is a Feed now?
- (a) A simple ordered list of items (events? messages? posts?) owned by a parent entity.
- (b) A separate entity referenced by the parent, with its own identity and lifecycle.
- (c) Still a saved query — but now scoped to a parent entity rather than universal.
- (d) Something else.

> ✓ **Answered: renamed to Chain.** A Chain is an ordered sequence of Blocks, owned by a parent entity. Closest to (a), with the new name and explicit Block typing. Theme: a line of Blocks is a Chain.

### Q9. Aggregate (the DDD term) survives?

Keep "Aggregate" + "no FK between aggregates" rule for the four first-class citizens, or replace with simpler "first-class citizen" wording and drop the formal DDD framing?

> We can keep it. First-Class Citizens can live together in our semantics.

### Q10. Tag — what to do with the shipped code?

Tag has working code in `domain/src/tag.rs`, `entity_tag` junction migrations, `application/src/tag/`, `api/src/routes/tags.rs`. With Tag deferred, do we:
- (a) Leave the code in place; stop building on it; revisit when Tag returns to scope.
- (b) Remove all Tag code (everything, including the migration's `entity_tag` table) to keep the codebase lean.
- (c) Keep schema + persistence; remove application + API layers (so the tables exist but no service logic).

> Remove all Tag code. It is going to change in the future. This code is now deprecated.

### Q11. Output / Product

The publishable artifact a Commission produces. Is it:
- (a) Its own entity (a `CommissionOutput` aggregate?) referenced by the Commission.
- (b) A specific feed contained by the Commission ("output" feed).
- (c) A blob attached to the Commission.
- (d) Just a public Record (i.e. a published Block) by the artist, optionally referencing-back to the (private) Commission.

> It is it's own output, contained by the Commission. Usually, it will be a blob by itself, in the output feed (Blob in a Block). 

### Q12. Name for the feed-composition primitive

The recursive thing inside a Feed: references other instances of itself, mounts Blobs, carries typed sub-shapes (your example: inline text + image + inline text — recursive JSON shape per `at://`). Confirmed shape: **same primitive across public and private** — visibility is *where-it-is-stored*, not *what-shape-it-is*. Public version serializes to a Document/Record on atproto (Lexicon-defined); private version stays as JSON in our DB.

The need: `Document` got conflated with the public-storage shape, so the semantic primitive needs its own name. Candidates:

- (a) **Block** — Notion-flavored. Concrete, recursive, fits "text + image + text" naturally. Strongest editor-shape prior art (Notion, Editor.js, Slack message blocks). Risk: overloaded ("block list," "block as obstruct").
- (b) **Composite** — DDD-literal; ties directly to your "composition is resolved through references." Risk: more abstract / less concrete.
- (c) **Element** — already exists in code as `feed_element` (currently a flat, non-recursive sub-part of `feed_item`). Elevation candidate. Risk: HTML/DOM connotations; conflicts with the current narrower code meaning.
- (d) **Component** — React-flavored, matches your earlier "React-Esque pieces of data" remark. Risk: heavily overloaded in software (UI components, etc.).
- (e) Something else.

Claude's lean (offered, not decided): **Block** — most concrete, most familiar editor-shape, doesn't presuppose visibility or storage.

> ✓ **Answered: Block.** Block is the lowest possible compositable element. Themed semantics: **Blob** → **Block** → **Chain**.

Confirm: same primitive across both visibilities (visibility = storage location, not shape)?

> ✓ **Confirmed.** Same primitive both ways. Public Block serializes to an atproto Record (Lexicon-defined); private Block stays as JSON in our DB.

---

## Pointers to other docs

- `SCOPE.md` — feature-by-feature MVP vs. roadmap split with cross-cutting redesign questions.
- `NEW_DESIGN.md` — architectural philosophy log; partially superseded by your Object answer here.
- `VISIBILITY.md` — TBD; full Public/Private rules go here.
- `design/infrastructure/layers/DESIGN.md` and `LAYER_RULES.md` — layer architecture; settled.
- `design/glossary.md` (lowercase) — historical; now contradicted on Personal-Org rename, Character-ownership flip, and "Everything is an org." Kept as reference; rewrite once this glossary settles.
