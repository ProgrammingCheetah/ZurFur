# Feature 11: Organization TOS — Design Questions

Answer inline after each question. When done, tell me to check this file.

---

## Entity System

**Q1: Should TOS be an Entity?**
With the "everything is an entity" system, should `OrgTos` implement `Entity`, `Taggable`, `FeedOwnable`? This would give it an `EntityKind::OrgTos` variant. Implications:
- Tagging a TOS version (e.g., `metadata:refund-friendly`, `metadata:strict-no-refund`) could power search/discovery
- TOS owning a feed could enable change notification streams
- Or TOS is an owned child of Organization (like Character) and doesn't need its own entity kind

**Q2: TOS as feed items?**
Org bios are stored as feed items — edits are new feed items, giving version history for free. Should TOS versions follow the same pattern? Each version would be a feed item in a system `tos` feed attached to the org.

Pros: consistent with the architecture, version history via feeds, change notifications for free.
Cons: TOS has structured sections (JSON), not freeform content. Feed items are chronological events, TOS is a document with revisions.

Or should TOS stay as its own table (`org_tos`) with explicit versioning?

---

## Content Format

**Q3: Structured JSON or markdown?**
The README says structured JSON with standard section keys (`refund_policy`, `usage_rights`, etc.). But markdown is simpler and more flexible. Options:
- **Structured JSON** — predefined section keys, easy to diff, easy to template
- **Markdown** — freeform, the artist writes what they want, harder to diff structurally
- **Hybrid** — standard sections stored as JSON, each section body is markdown

**Q4: Are the 6 standard sections correct?**
From the README: `refund_policy`, `usage_rights`, `turnaround_time`, `communication`, `revisions_policy`, `payment_terms`. Should any be added or removed? Can orgs add custom sections beyond these?

---

## PDS Publishing

**Q5: Do we implement PDS publishing now?**
The README says active TOS is published to PDS as an AT Protocol record. We don't have PDS write support built yet. Options:
- **Implement now** — add PDS write capability as part of this feature
- **Stub it** — add a `pds_record_uri` column, leave it null, implement publishing when AT Protocol integration is built (Feature 1.2-1.4)
- **Skip it** — don't even add the column, add it later

**Q6: What AT Protocol lexicon should TOS use?**
If we publish to PDS, we need a record type. Options:
- Custom lexicon: `app.zurfur.tos.version`
- Or defer the lexicon design entirely until AT Protocol integration

---

## Permissions & Scope

**Q7: Who can create/publish TOS versions?**
`MANAGE_TOS` (bit 4) already exists. Should it be restricted to owners only, or any member with `MANAGE_TOS`?

**Q8: MVP scope — just Phase 1?**
The README defines 3 phases. For our WORKBOARD Phase 4A, should we:
- **Phase 1 only** — TOS CRUD + versioning (no acceptance, no diff, no PDS)
- **Phase 1 + acceptance stub** — TOS CRUD + `tos_acceptances` table + acceptance endpoint, but no commission gate yet (that's Feature 4)
- **All 3 phases** — everything including diff

---

## Versioning

**Q9: How should version numbers work?**
Options:
- **Auto-increment integer** — simple, `version = 1, 2, 3...`
- **Timestamp-based** — version IS the created_at timestamp
- **Content-hash** — SHA of content_json for dedup detection

**Q10: What happens to active commissions when TOS changes?**
If an artist publishes a new TOS version mid-commission:
- **Nothing** — the commission was accepted under the old version, that's binding
- **Notification** — notify the client that TOS changed, but don't require re-acceptance for in-progress commissions
- **Re-acceptance required** — block further actions until client accepts new TOS (disruptive)
