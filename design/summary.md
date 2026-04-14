# Zurfur — Platform Summary

## What We Are Building

Zurfur is a decentralized art commission platform built on the AT Protocol (the technology behind Bluesky). It consolidates the fragmented tools that artists and clients currently juggle — Trello for tracking, PayPal for payments, Telegram for communication, Google Sheets for queues, FurAffinity for discovery — into a single, unified platform where the user owns their data.

The platform acts as a secure intermediary between artists and clients. It handles the entire commission lifecycle: discovery, intake, tracking, communication, payment, delivery, and reputation. Unlike existing platforms, Zurfur does not own the user's identity, galleries, or social graph. Public data lives on the user's AT Protocol Personal Data Server (PDS), portable and independent of Zurfur's continued existence.

## The Problem

The furry art commission market generates an estimated $30-100M+ annually, yet operates almost entirely through ad-hoc toolchains:

- **Discovery** happens across scattered galleries (FurAffinity, DeviantArt, Twitter/Bluesky, personal sites) with no unified search
- **Intake** is handled via Google Forms, Trello cards, or DMs with no standardization
- **Payment** flows through PayPal, Venmo, or Ko-fi with no escrow protection for either party
- **Communication** happens on Discord, Telegram, or email with no audit trail tied to the commission
- **Tracking** is artist-specific (spreadsheets, Trello boards) with no client visibility
- **Reputation** is informal (word of mouth, beware lists) with no verifiable transaction history
- **Disputes** have no platform-level resolution — chargebacks go through payment processors who lack context

This fragmentation creates friction for everyone. Artists waste hours on administration instead of creating. Clients risk fraud with no recourse. Both parties lack transparent data to make informed decisions about pricing, timelines, and trustworthiness.

## Target Audience

**Primary: Furry artists and commissioners.** This community is the beachhead market because:

1. **Highest commission volume** — furry art is one of the most active commission markets on the internet, with thousands of transactions daily
2. **Underserved by mainstream platforms** — NSFW content restrictions on app stores and social media force this community to the margins of existing tools
3. **Strong community identity** — the furry fandom has tight-knit social networks, convention culture, and shared vocabulary that make targeted platform design effective
4. **Early tech adopters** — the community was among the first non-tech audiences on Bluesky, making AT Protocol integration a natural fit rather than a novelty
5. **Complex workflow needs** — commissions involving original characters, reference sheets, multi-step pipelines, and custom TOS require purpose-built tooling

**Secondary: Any creator community with commission workflows.** The platform's architecture (org-centric identity, headless commission engine, plugin ecosystem) is not furry-specific. Cosplay makers, VTuber riggers, music producers, and tabletop miniature painters all face similar problems. The furry community validates the model; expansion follows.

## Core Concepts

### Everything is an Org with Feeds

The platform has two universal abstractions:

- **Organization** — the identity container. Solo artists, multi-member studios, and plugins all use the same entity. A user's "profile" is their personal org. There is no `is_artist` flag — "artist" is a role on an org.
- **Feed** — the content container. Galleries, activity logs, commission histories, notifications, and chat threads are all feeds rendered with different templates. Adding a new view requires creating a feed and a template, not new backend endpoints.

### Headless Commission Engine

Commissions are minimal shells with artist-defined states and plugin-based add-ons. The platform does not prescribe workflow — it provides the infrastructure for any workflow. Kanban boards are projections over commissions, not owners of them.

### Data Sovereignty

Public identity data (profiles, galleries, social graph) will live on the user's AT Protocol PDS. Private transaction data (payments, disputes, chat) stays in Zurfur's database. The user can leave and take their identity with them.

## Make-or-Breaks

These are the factors that will determine whether Zurfur succeeds or fails:

### Must Get Right

1. **The critical path must work end-to-end.** A client must be able to discover an artist, submit a commission, pay, track progress, receive delivery, and leave a review — seamlessly. If any link in this chain is broken or clunky, users will return to their ad-hoc toolchains.

2. **Trust as a financial intermediary.** Holding money between parties is a serious responsibility. Dispute resolution, chargeback handling, and fraud prevention must be robust from day one. A single high-profile payment failure could destroy credibility in a community that spreads news fast.

3. **Artist adoption.** Artists are the supply side. If they don't see immediate, concrete value over their current Trello + PayPal + Discord setup, they won't switch. The onboarding experience must demonstrate time savings within the first session. The platform must respect artists' existing workflows, not demand they learn a new one.

4. **Content moderation at scale.** A platform that supports NSFW content must have clear, enforceable policies and tooling from the start. Legal liability (FOSTA-SESTA, international regulations), community trust, and payment processor compliance all hinge on this.

5. **Feed infrastructure performance.** Feeds are the backbone of everything. If feed queries are slow, pagination is janky, or real-time delivery lags, the entire platform feels broken. This is the foundational infrastructure that every feature depends on.

### Critical Risks

6. **Scope creep.** The design document describes a "super app" with 14 features, a plugin ecosystem, AI analytics, and AT Protocol federation. Building all of this before validating the core commission flow is a path to failure. The critical path (auth → orgs → tags → feeds → TOS → commissions → payments) must be completed and validated before anything else.

7. **AT Protocol maturity.** The AT Protocol is still evolving. Building on it provides long-term advantages but introduces short-term risk: API changes, missing features, and a small ecosystem. The two-tier architecture mitigates this by keeping private data in PostgreSQL regardless.

8. **Payment processor relationships.** Stripe and other processors have restrictions on adult content. Maintaining a payment processing relationship while supporting NSFW commissions requires careful compliance work and may limit which processors are available.

9. **Network effects.** A commission platform is a two-sided marketplace. It needs both artists and clients. Cold-start strategies (targeting convention communities, partnering with popular artists, offering migration tools from existing platforms) are essential.

## Current State

- **Feature 1.1 (OAuth):** Complete. Users can authenticate via Bluesky.
- **Feature 2 Phase 1 (Identity):** Complete. Org-centric model, personal orgs, memberships, permissions.
- **Feature 2 Phase 2 (Feeds + Onboarding):** Domain + persistence layer done. Application layer next.
- **Everything else:** Not started. Tags, TOS, commissions, payments, plugins, search, notifications, moderation, and admin are all ahead.

The foundation is solid. The architecture decisions (org-centric identity, feed-based content, headless commissions, two-tier data) are sound and well-documented. The challenge now is executing the critical path efficiently without getting pulled into premature optimization or feature expansion.
