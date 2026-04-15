# Feature 2 Phase 3: Characters — Design Questions

Answer inline after each question. When done, tell me to check this file.

---

## Entity Design

**Q1: Character feed — what slug and type?**
Orgs get system feeds like "updates", "gallery", "commissions" via OnboardingService. What should the character's auto-created feed be called? Slug "gallery"? Feed type "system" (undeletable)?

> Yep. A character's relationship with it's feed is unable to be deleted. A character ALWAYS has a feed, even if said feed is empty. To show Character "Previews", ref_sheets, etc. we just choose stuff from the character feed and interpret them. A profile picture is the only case of the character not needing to use it's own feed, unless we define it that way. Ref sheets are just feed elements that have been tagged with ref_sheet and the like.

**Q2: Character name constraints?**
Any length limits (min/max characters)? Allowed character set (Unicode? ASCII only?)? Can names be changed after creation?

> Names can certainly be changed after creation. I think we should limit it to a max of... 256 characters? It does not really matter that much. However, we should support any characters, even Emojis. Thanks to the fact that tags point at the `entity_id`, we will have no trouble with name changes. These do NOT require us keeping history.

**Q3: Character description — format and limits?**
Plain text only, or markdown? Max length?

> I think we should use markdown in a TEXT format. People sometimes love being very descriptive about their characters.

**Q4: Default values?**
- Default `content_rating`: Sfw?
- Default `is_public`: true (visible to everyone) or false (only org members see it)?

> We should not have a default `content_rating`. It should be an explicit choice by the user. We should also have a `visibility` clause: If `private`, only org. If `public`, anybody. If `controlled`, people allowed to see it explicitly (through another permissions table, either user or orgs can be added here). If `unlisted`, anybody with a link but it won't appear in a profile. 

**Q5: Character limit per org?**
Should we enforce a maximum number of characters per org? Or unlimited for now?

> Nah. No maximum number.

## Permissions & Visibility

**Q6: Who can view non-public characters (`is_public: false`)?**
Only org members? Members with a specific permission? Or is `is_public` just a frontend hint and the API always returns them?

> Already answered this one in Q4. It should be an enum.

**Q7: Who can attach/detach tags on a character?**
Same `MANAGE_PROFILE` permission as CRUD? Or should any authenticated user be able to tag (community tagging model)?

> An Org may allow to: `community tagging model`, `approval-based community tagging model` or `locked` (only org)

**Q8: Should the top-level GET /characters/:id respect is_public?**
i.e., return 404 for non-public characters when the requester isn't an org member? Or always return regardless?

> Yep. 404. Character if unlisted or public or allowed. 404 otherwise.

## Deletion & Cascading

**Q9: What happens to a character's feed when the character is soft-deleted?**
Leave feed accessible? Soft-delete the feed too? Or just hide the character from listings and let the feed become orphaned?

> Feed becomes unaccessible without the character. They come as a package.

**Q10: What happens to a character's tags when soft-deleted?**
Detach entity_tags? Decrement usage_count? Or leave them (they become orphaned references)?

> In the case of an artwork being tagged with them, they are STILL related, but they are just not shown to the world. If the character comes back, then they can be shown again. The usage_count for the tag won't matter if the character was straight up soft deleted.

## API Shape

**Q11: Character response — flat or enriched?**
Should `GET /characters/:id` return just the character fields, or also include tags and feed info? Options:
- **Flat:** Just character fields. Tags and feeds fetched via separate endpoints (`/characters/:id/tags`, `/characters/:id/feeds`).
- **Enriched:** Character + tags + feed ID in a single response. Fewer round trips.

> Flat. I think TTI is more important. We can load smaller things faster while waiting for more data to arrive. We can package different things but we should strive for parallel fetching.

**Q12: Listing filters?**
Should `GET /orgs/:id/characters` support query params for filtering (content_rating, is_public, tag search)? Or just basic `limit`/`offset` pagination for now?

> Yes! It should be `/orgs/:id/characters?limit|rating|tags[ARRAY]|offset|etc.`

**Q13: The org parameter in nested routes — confirm behavior:**
`/orgs/:org_id_or_slug/characters` resolves org by UUID first, then slug. The existing org routes already do this. Should character routes reuse the same resolution helper? (I believe yes, just confirming.)

> I think we should solve for slug first. Slugs are going to be copy-pasted more frequently.

## Ref Sheets (for future reference)

**Q14: How should ref sheet items be distinguished within the gallery feed?**
You mentioned tagging feed items with a `ref_sheet` tag. Is this a metadata tag (`TagCategory::Metadata`)? Or would you prefer a different mechanism — like a field on feed_item or a specific feed_element type?

> That would be a metadata, yes. So it would be a `TagCategory::Metadata` with the name `ref_sheet`.



## Some Notes
> Characters have the same logic attached to them as Organizations. They belong to an org (Most common use case will probably be a user), and their gallery is just a feed. This gives us immediate flexibility for the future: featured arts would just be marked (in the feed?) as featured through tags; characters themselves can be tagged as well; and the feed would also trigger org feeds if wanted. The only intrinsic things we need in a character are identifiable things, such as a description, which the system does NOT care about. I recommend we consider using markdown. If we want to follow the domain rules, this could just be a one-item feed that allows for a description and the usage of templates. What do you think? We could 100% make it more customizable in the future, since we would only need to change feed elements altogether. CSS and everything should be considered way down the line rather than right now, because they present such problematic security issues. Likewise, I strongly believe that characters should not be able to be hard deleted unless specified with a button. Hard deletion of characters deletes their character tags from all art work, and all data is lost.
