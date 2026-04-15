# Entity Interfaces — Design Questions

Answer inline after each question. When done, tell me to check this file.

---

## Core Entity Interface

**Q1: Should `Entity` be a Rust trait or a documented concept?**
Options:
- **Rust trait** — `trait Entity { fn id(&self) -> Uuid; }` enforced by the compiler. Every domain struct implements it.
- **Documented concept** — no code-level enforcement, just a design principle. Entities are structs with an `id: Uuid` field by convention.
- **Marker trait** — `trait Entity {}` with no methods. Used for type bounds but no shared behavior.

> Both, actually. It is an entity, so it has to implement the ID (As a UUID) AND be documented as a consequence. 

**Q2: Should every entity be Taggable by default?**
Right now, Users are NOT taggable and Feeds are NOT taggable. Two options:
- **Opt-in** — entities explicitly implement Taggable. Some entities (User, Feed) choose not to. This is what we have today.
- **Default yes** — every entity is taggable, but some just don't have tags attached yet. The capability exists even if unused. More flexible, matches "features are combinations."

> Technically, users can be tagged and feeds can be tagged, as per our domain rules. Both can be queried through tags. The default yes is better.

**Q3: Should every entity be FeedOwnable by default?**
Same question for feeds. Right now, Tags and Feeds don't own feeds. Options:
- **Opt-in** — entities explicitly implement FeedOwnable. Tags and Feeds don't.
- **Default yes** — every entity CAN own feeds. Tags just don't have one yet. When you want tag feeds (topic following), you just create one.

> Tags owning feeds and feeds owning feeds do not make sense for now, but they should implement it anyways, even if does nothing. We should also add logging to see what is happening.

---

## FeedOwnable

**Q4: Does FeedOwnable imply a default system feed on creation?**
Organizations get system feeds auto-created (updates, gallery, activity). Characters get a gallery feed. Should this be a contract of the interface — "if you're FeedOwnable, you get at least one system feed on creation" — or is it entity-specific logic?

> No, that is a rule that should only be enforced on stateless, as per domain rules. Organizations get a feed because there is an action that happens once that makes it.

**Q5: Can an entity own feeds belonging to another entity?**
Example: an Organization "manages" a Character's gallery feed because the character belongs to the org. Is feed ownership always direct (entity_feed junction), or can it be inherited through relationships?

> I think it should be direct. We can have permissions added later for inheritance. Add to ROADMAP and Notion.
---

## Taggable

**Q6: Should Taggable enforce anything about tag categories?**
Right now, orgs get `TagCategory::Organization` tags and characters get `TagCategory::Character` tags (auto-created, immutable). Should the Taggable interface define which categories apply, or is that service-layer logic?

> I do not understand, but I think you mean where the relationship is created. This is always user-oriented. We should add an `is_locked` so that only platform admins can modify anything about them.
---

## Authorable

**Q7: Is Authorable only for "actors"?**
Current authors: User, Org, System. These are things that take action. Should Characters ever be able to author feed items? Or is authorship always traced back to a user or org acting on behalf of the character?

> Technically, yes. But, semantically, no. On Syntax, a character could author a feed, but on semantics, it makes no sense. Things are posted on a character but authored by the parent organization.

---

## EventEmitter

**Q8: Is EventEmitter the same as FeedOwnable, or separate?**
You described entities generating events (`event.feed.TYPE.SUBTYPE`). Two interpretations:
- **Same as FeedOwnable** — owning a feed IS emitting events. The feed is the event stream.
- **Separate capability** — an entity can emit events into feeds it doesn't own. A Character creation event appears in the Org's activity feed, not the Character's feed.

Which is it? Or both — FeedOwnable means "has its own event stream," EventEmitter means "can produce events that land in other streams"?

> Technically, anybody can emit into any feeds. However, this is also bad for security and ownership. I think we should "sign" things and the feed is the event stream. We only save the way to "name" it, per se.

**Q9: Who decides where events land?**
When a Character is created, events might go to:
- The Character's own gallery feed
- The Org's activity feed
- The Org's updates feed (if configured)

Is event routing part of the entity interface, or is it application-layer logic (services decide where to post)?

> The character creates it's own gallery feed. Adds an ID or something that means "This event was this" and bubbles it up. Orgs puts it in it's feed. To prevent duplication on showing it, we simply check the ID in the AT Protocol. It creates redundancy, yes, but it keeps the feeds perfectly separate. If you have any better solution for this, I am willing to listen to it. Another way would be for Characters to be filters of the Organization's feed again. This means that a character's gallery is just a filtering of the Organization's feed, which is simpler and follows our Domain Rules better (I also like this more).

**Q10: Should the event structure (`event.feed.TYPE.SUBTYPE`) be formalized in the domain?**
Options:
- **Enum-based** — `EventType::Lifecycle(LifecycleEvent::Created)`, `EventType::Character(CharacterEvent::ArtUploaded)`. Compile-time safety.
- **String convention** — `"event.feed.lifecycle.created"`. Flexible, no migration needed for new event types. Matches AT Protocol lexicon style.
- **Hybrid** — Rust enum internally, serialized as dotted string for feeds and AT Protocol.

> String convention. Anybody can create their own thing. Decentralized. We can have our own things we look for, but only string convention. This is why we act as a reaction + index rather than a look up every single time.

---

## Subscribable

**Q11: Is Subscribable derived from FeedOwnable?**
If you can own feeds, others can subscribe to those feeds. So Subscribable = "has feeds that can be subscribed to." Does it need to be a separate interface, or is it just a consequence of FeedOwnable?

> I think it is just a consequence. We can talk more about this. You subscribe to a feed. Subscribing to an organization is just subscribing to their default feed. 

**Q12: Can non-FeedOwnable entities be subscribable?**
Could you subscribe to a Tag even if the Tag doesn't own a feed? This would mean "subscribe" is really "create a query-based virtual feed" rather than "subscribe to an existing feed." Is that in scope, or is subscribing always to a concrete feed?

> Subscription is a concrete feed. Tags with a feed require a feed. However, I think in the case of the implementation for Tag (Given we are using an interface), we would only be creating a query-based behind the interface every time it is looked up. What I am saying is that Tags do not have a table to save their feed, but rather is a virtualized query (Hence why the interface is so powerful).

---

## Scope

**Q13: Are we defining these interfaces for documentation, or implementing them as Rust traits?**
This affects how deep we go. If documentation-only, we define contracts and move on. If Rust traits, we need to think about:
- Generic bounds on services (`fn create<E: Entity + Taggable>(...)`)
- Trait object compatibility (`Arc<dyn Entity>`)
- Whether existing structs need refactoring

> We are implementing them as Rust traits. They are the most important part of our architecture. This should be our main goal right now.

**Q14: Should we formalize these interfaces before or after OpenAPI?**
OpenAPI documents the API layer. Entity interfaces define the domain layer. They're independent, but the entity interfaces might influence how we document schemas in OpenAPI. What's the priority order?
> Before. That way we don't work twice on the OpenAPI.
