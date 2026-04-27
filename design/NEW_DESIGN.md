# New Design

Working document for the architectural philosophy shift. Where this conflicts with `design_document.md`, this document takes precedence. Once the shift is settled, the relevant parts of `design_document.md` get rewritten and this scratch doc retires.

## Changes in Domain
We are now going to introduce the concept of **Documents**.

### Feeds
Feeds change a little bit. Feeds are named, persistent and append-only, time-ordered, subscribable projection over **Documents**. They are NOT a container, but rather a saved query. 

### Documents
Documents are authored, addressable, fact-bearing composition of other documents. They are meant to be polymorphic.

Documents may be a leaf when they contain no children, or they may be a composition. Documents may refer other documents or contain Blobs.

Documents themselves can be text and be referenced. 

### Blobs
Blobs are opaque bytes, content-addressed by a CIDs. Content hashes are advised. Blobs are referenced by Documents.

Blobs are the smallest unit of **content**. They hold no identity, authorship, or behavior. They are ONLY media.

## Open Questions

### Document Addressing

Documents are "addressable" — by what scheme?

- **UUID** — mutable identity; edits keep the same ID. Simple, but no content integrity.
- **CID** — immutable; editing creates a new Document. Versioning is built-in, but renaming/refining a "thing" becomes hard.
- **AT-URI-shaped** (`{author_did}/{collection}/{rkey}` plus per-revision CID) — mutable address with content integrity per revision. Atproto-native; most ceremony.

> AT-URI-SHAPED with per-revision CID.
### Polymorphism Mechanism

How are Document types defined?

- **Lexicons** (atproto's JSON Schema record-type definitions, referenced by `$type`) — Zurfur Documents and atproto records become structurally identical. Highest upfront cost; zero translation when publishing to the network later.
- **Internal type field + JSON schema** — faster v1; later atproto interop becomes a translation layer.

> Lexicons. 

### Document Boundary

Where is the line between "Document" and operational state? Items on the seam:

- A **commission** — is it a Document, or a row that *owns* a commission-spec Document plus pipeline/payment/timestamps as operational state?
- A **tag** — is it a Document, or a lighter-weight annotation attached to Documents? (Atproto has a separate "label" concept for this — small, often third-party-applied annotations like `{labeler_did, target_uri, value, signed_at}` — distinct from records.)
- **Org settings** (open/closed, ToS template, payout info, subscription tier) — Documents, or operational rows?

> everything that crosses the federation boundary is a Document. Everything purely operational stays as DB rows. Commission spec? Document. Commission pipeline state, internal timestamps, payment processor refs? Row. Public character profile? Document. Tags specifically: AT Proto has labels for exactly this — small, signed annotations distinct from records. Don't model tags as Documents; model them as label-shaped objects. Atproto has solved this already. The split is: federatable + bilateral fact = Document. Process state + sensitive ops = row.

### Append-Only Semantics

If a Feed is a saved query, then strictly speaking new matches arrive and old non-matches disappear as Documents change. So "append-only" must mean something specific. Which?

- **Documents themselves are append-only** — no destructive edits; revisions create new versions; the timeline of a Document's history is monotonic.
- **Feed positions are append-only** — a Document's slot in a feed is fixed once assigned, even if the underlying Document is later edited or deleted.
- Both? Neither (and the "append-only" wording needs revising)?

> Both, but with precision. The wording "append-only" is doing too much work. Better:

Documents are immutable per revision. A given AT-URI + CID pair is forever. Editing produces a new CID at the same AT-URI. The Document's revision history is monotonic.
Feeds are projections, not append-only logs. A feed is query-as-of-now. If a Document is superseded, it leaves the projection. If a new Document matches, it appears. Feed positions are not fixed.
The underlying event stream IS append-only. Every create/edit/supersede is an event, and events never disappear.

So the precise statement: events are append-only, Documents are immutable-per-revision, Feeds are projections over the event stream.


### Publication Boundary

Documents conceptually fit on atproto, but atproto records and blobs are **public by design**. CIDs give content integrity, not confidentiality — a CID is also a fetch handle, and the PDS serves the bytes to anyone who asks. Storing a private Document on atproto exposes it (and even hashes alone leak low-entropy content via dictionary attack).

So which Document types publish to atproto, which stay Zurfur-internal?

- **Public-by-intent** (candidates for atproto): Open Comms announcements, public character profiles, commission slot listings, public posts.
- **Private** (candidates for Zurfur-internal storage with private object storage + app-layer access control): commission specs and drafts, contract instances, private character profiles, payment metadata.
- **Hybrid** (atproto-published metadata, encrypted-blob payload, or selective fields): possible but adds key management.

> Documents in Zurfur fall into two storage regimes, determined entirely by whether their content is intended to be public. Public-by-intent Documents — open commission slot listings, public character profiles, finished portfolio entries the artist has chosen to share, and announcement-style posts — are published to the user's PDS as AT Proto records under the com.zurfur.* Lexicon namespace, with any associated media stored as AT Proto blobs on the same PDS. Once published, these Documents are federated through the firehose and readable by any consumer; Zurfur's database holds an index over them for fast queries, but the user's PDS is the source of truth. Private Documents — commission specs, contract instances, private chats, payment metadata, and private character profiles — live exclusively in Zurfur's own database, never touching AT Proto. Their associated media is stored in Zurfur's private object storage and served via short-lived signed URLs to authorized parties only. Hybrid encryption-on-AT-Proto schemes are explicitly avoided; the protocol team has stated that bolting encryption onto existing primitives is wrong, ciphertext stored on a federated public log has long-tail exposure risks, and metadata leaks (record sizes, communication timing, party DIDs) reveal sensitive inferences even when contents are encrypted. The governing rule is that nothing goes on AT Proto unless the user would be comfortable posting it on their public BlueSky timeline; everything else stays in Zurfur's private storage until the AT Proto private-data working group ships native support for non-public records.

## Follow-up Questions

### Addressing for Private Documents

Public Documents have AT-URIs (`{author_did}/{collection}/{rkey}`) hosted on the user's PDS. Private Documents never touch atproto. Do they share the AT-URI shape or use a different scheme?

- **Uniform AT-URI grammar** — private Documents use AT-URI form even though they're never pushed to a PDS. Schema is consistent across both regimes; addresses for unpublished items feel slightly fictional.
- **UUID for private** — private Documents have their own ID scheme. Storage and addressing line up (no atproto = no AT-URI), but two ID conventions coexist in the codebase.
- **Custom namespace** (e.g. `at://zurfur.private/{type}/{id}` or similar) — same grammar, distinct semantics signaling "this never federates."

> We should use custom namespace. Otherwise, the UUID is going to introduce two different ways of parsing the same thing and the uniform AT-URI grammar is just plain lying. We can discuss about the namespace's shape later.


### Event Log Location & Ownership

Your append-only refinement defines events as the primary source: "events append-only, Documents immutable-per-revision, Feeds projections over the event stream." Where does the event log actually live?

- For **public Documents**, the user's PDS already maintains a commit log (the firehose). Zurfur subscribes to it and projects into local indexes. PDS = source of truth.
- For **private Documents**, no PDS is involved, so Zurfur owns the event log itself — an outbox table or equivalent that records each create / edit / supersede.

Confirm both? And: are the two event streams unified at the consumer level (one logical stream Zurfur reads from, regardless of source), or kept distinct because they have different durability/auth properties?

> Correct to both. Two physical streams, one logical interface at the consumer. 

### Partially-Public Material

Encryption-on-atproto is off the table, but some material is naturally split: a commission has a *public* "I'm taking commissions of this slot type" listing, while the actual spec and reference pack are *private*. How is this modeled?

- **Two Documents linked by reference** — a public summary Document on the PDS (federated) plus a private spec Document in Zurfur's DB. The summary references the spec by an opaque local ID; consumers of the public side never resolve into the private side.
- **One Document with two projections** — a single private spec lives in Zurfur, and a derived public-safe summary is generated at publish time and pushed to the PDS as its own record. The summary is a *view* of the spec, not a peer Document.

The choice determines whether the spec/summary relationship is a stable Document-to-Document reference or a one-way projection.

> Commissions and post are completely separate things. The "I'm taking commissions of this slot type" is a post made somewhere else. A commission is created completely separate.


### Tag Implementation

You've decided tags use AT Protocol's implementation. Atproto has two distinct tag-like primitives — pick which (or both):

- **`tags` field on records** — array of strings embedded directly on a Document (e.g. `app.bsky.feed.post.tags = ["art", "wip", "dragon"]`). Self-applied by the author. Indexed by atproto for hashtag-style search. Natural fit for self-categorization: an artist tags their own character with `dragon`, `fantasy`, `oc`.
- **Labels** (`com.atproto.label`) — separate signed records applied *to* a target Document by a labeler (itself an atproto actor). Natural fit for third-party annotation: community moderation, content warnings, opinion-bearing categorization. This is what your earlier Document Boundary answer pointed at.
- **Both** — `tags` field for self-categorization, labels for third-party/moderation annotation. They serve different purposes and don't conflict; most atproto-shaped apps use both.

> I would say both. It makes sense. Tags are easy to query. I wouldn't even put them in the AT Protocol if they are harder to query (We will index this anyways, so I think they can go in there). Tags will be able to be placed on users, commissions, commission outputs and characters. They mostly have meaning inside the app but exist in the AT Protocol for decentralization. Labels are... Labels, like you said.


### Tag Targets — Per-Type Mechanism

Tags target users, commissions, commission outputs, and characters. Two of these don't fit the `tags`-field-on-record model directly:

- **Users are DIDs, not Documents.** A DID has no record to embed a `tags` field on. Options:
  - **Labels** — atproto labels can target a DID directly, signed by a labeler. Means Zurfur operates as a labeler (or delegates to a designated one). Tags-on-users federate.
  - **Local-only** — Zurfur stores user-tag rows internally, no federation. Simpler; no decentralization for this subset.
- **Commissions and commission outputs are private Documents.** Their `tags` field lives on the local record, which never goes to the PDS — so those tags don't federate either. Consistent with the publication boundary; just narrows "tags exist in AT Proto for decentralization" to *public*-Document tags only.

Per-target picture:

| Target | Mechanism | Federates? |
|---|---|---|
| Document (public — e.g. character profile, post) | `tags` field on Document | yes |
| Document (private — e.g. commission, output) | `tags` field on local record | no |
| User (DID) | Labels *or* local-only | depends on the choice |

For users specifically — Labels (Zurfur as labeler) or local-only?
