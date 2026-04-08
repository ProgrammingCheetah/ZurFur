# Feature 4: Financial & Payment Gateway

## Overview

Zurfur acts as the merchant of record — clients pay the platform, the platform holds funds and issues payouts to artists. This escrow-lite model minimizes chargeback fraud and builds trust. Supports flexible invoicing, installment plans, and voluntary fee coverage.

## Sub-features

### 4.1 Platform Intermediary (Escrow-Lite)

**What it is:** Zurfur is the payment processor's customer (merchant of record). Clients pay Zurfur; Zurfur pays artists on milestones/completion.

**Implementation approach:**
- **Stripe Connect** (Standard or Express accounts) for artist onboarding
- Artists complete Stripe KYC as connected accounts
- Payment flow: client → Stripe checkout → funds held in Zurfur's Stripe balance → payout to artist's connected account on milestone/completion
- `artist_payment_accounts` table: `user_id`, `stripe_account_id`, `onboarding_status`, `payout_enabled`
- `transactions` table: `id`, `commission_id`, `invoice_id`, `type` (charge/refund/payout), `amount_cents`, `currency`, `stripe_payment_intent_id`, `status`, `created_at`
- `payouts` table: `id`, `artist_id`, `amount_cents`, `stripe_transfer_id`, `status`, `created_at`
- Webhook handlers: `payment_intent.succeeded`, `charge.refunded`, `transfer.paid`, `account.updated`

### 4.2 Flexible Invoicing

**What it is:** Multiple invoices per Commission Card, each independently payable.

**Implementation approach:**
- `invoices` table: `id`, `commission_id`, `artist_id`, `client_id`, `amount_cents`, `currency`, `description`, `status` (draft/sent/paid/cancelled), `due_date`, `paid_at`, `stripe_checkout_session_id`
- Artist creates invoice via API → client receives notification → client pays via Stripe Checkout
- Payment triggers `PaymentReceived` event on the Card
- API: `POST /commissions/:id/invoices`, `GET /invoices/:id`, `POST /invoices/:id/pay`

### 4.3 Installments & Subscriptions

**What it is:** Timed/automated billing cycles for large commissions.

**Implementation approach:**
- `installment_plans` table: `id`, `commission_id`, `total_amount_cents`, `installment_amount_cents`, `frequency` (weekly/biweekly/monthly), `next_due_date`, `remaining_installments`
- Background job: on `next_due_date`, auto-generate an invoice and notify the client
- Alternatively, use Stripe Billing for subscription-like recurring charges
- Plan creation: artist defines total, installment size, frequency when creating the commission

### 4.4 Voluntary Fee Coverage

**What it is:** Checkout toggle letting buyers absorb the platform transaction fee so the artist gets 100%.

**Implementation approach:**
- Calculate platform fee (e.g., 5% + Stripe processing fee)
- Display two prices at checkout: "Pay $X (artist receives $Y)" vs "Pay $X+fee (artist receives $X)"
- `cover_fee: bool` on the invoice/checkout session
- If opted in, increase the charge amount by the fee; if not, deduct fee from artist payout

## Dependencies

### Requires (must be built first)
- [Feature 1.1](../01-atproto-auth/README.md) — authenticated users
- [Feature 3](../03-commission-engine/README.md) — invoices attach to commission cards
- External: Stripe account, Stripe Connect setup, webhook endpoint configuration

### Enables (unlocked after this is built)
- [Feature 6](../06-plugin-ecosystem/README.md) — marketplace plugin purchases use payment infrastructure
- [Feature 7.3](../07-community-analytics/README.md) — financial data feeds pricing analytics
- [Feature 12](../12-dispute-resolution/README.md) — disputes freeze/release/refund funds

## Implementation Phases

### Phase 1: Stripe Integration & Basic Invoicing
- Stripe Connect artist onboarding flow
- `artist_payment_accounts` table + onboarding API
- `invoices` table + create/send/pay flow
- Stripe Checkout session creation
- Webhook handler for `payment_intent.succeeded`
- `transactions` table for audit trail
- `PaymentReceived` event emitted on Card
- Crates: domain (Invoice, Transaction entities), persistence, application (payment use cases), api (webhook endpoint, invoice routes)

### Phase 2: Payouts, Installments & Fee Coverage
- Payout logic: on commission completion, transfer funds to artist's connected account
- `payouts` table + Stripe Transfer API
- Installment plans with background job for auto-invoice generation
- Fee coverage toggle at checkout
- Refund flow (partial and full)

### Phase 3: Post-implementation
- Financial reconciliation reporting (daily summaries, discrepancy detection)
- Tax handling research (1099 for US artists, VAT for EU)
- Chargeback handling workflow (Stripe disputes → freeze card → alert artist)
- Load testing: webhook handler must handle burst of payment events
- PCI compliance review (Zurfur never touches raw card data — Stripe handles this)
- Documentation: artist onboarding guide, payment FAQ

## Assumptions

- Stripe Connect is the primary processor (most mature marketplace solution)
- Platform fee percentage is configured server-side (not hardcoded)
- Artists must complete Stripe KYC before receiving payouts
- All transactions are in USD initially (multi-currency is future expansion)
- Cryptocurrency support is deferred per the design document's roadmap

## Shortcomings & Known Limitations

- **Stripe geographic limitations:** Some countries not supported by Stripe Connect
- **Chargeback handling is complex** and partially manual — Stripe provides evidence submission tools but disputes require human review
- **Tax collection/reporting** (1099, VAT) not addressed in initial implementation
- **Escrow is "lite":** Not legally binding escrow — just platform-held funds. No escrow license.
- **No multi-currency support** initially
- **Refunds for installment plans** are complex — partial completion, partial payment, partial refund calculations
- **Payout timing:** Stripe standard payouts take 2-7 business days. Artists may want instant payouts (Stripe Instant Payouts, additional cost).
- **No PayPal integration** initially despite design doc mentioning it — Stripe Connect covers most use cases
