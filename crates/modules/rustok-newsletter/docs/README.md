# Newsletter Module Documentation

## Overview

The newsletter module provides cross-domain email campaign management for the RusToK platform. It is a standalone bounded context that aggregates content from multiple source modules (blog, forum, commerce) and delivers email campaigns to subscriber lists.

## Architecture

### Bounded Context

Newsletter is NOT a sub-feature of any source module:

- **Subscriber management** — opt-in/opt-out, double opt-in confirmation, suppression
- **Campaign lifecycle** — draft → scheduled → sending → sent / cancelled
- **Content aggregation** — source modules implement `NewsletterContentProvider` trait
- **Delivery** — uses `EmailDeliveryPort` from `rustok-email` as transport

### Cross-Module Integration

Source modules integrate through typed ports, not direct dependencies:

```
rustok-newsletter-api  ← contracts (ContentProvider trait, ports)
       ↑
rustok-newsletter      ← implementation (subscribers, campaigns, delivery)
       ↑
rustok-blog            ← implements NewsletterContentProvider
rustok-forum           ← implements NewsletterContentProvider  
rustok-commerce        ← implements NewsletterContentProvider
```

This follows the same pattern as `rustok-notifications-api` / `rustok-notifications`.

### Dependencies

- `rustok-email` — transport (`EmailDeliveryPort`)
- `rustok-outbox` — transactional event publishing
- `rustok-newsletter-api` — cross-boundary contracts

## Database Schema

### Tables

- `newsletter_subscribers` — subscriber records with status, confirmation tokens
- `newsletter_campaigns` — campaign definitions with lifecycle status
- `newsletter_subscriptions` — many-to-many subscriber ↔ segment mapping

### Indexes

- `idx_newsletter_subscribers_tenant_email` — unique email per tenant
- `idx_newsletter_subscribers_tenant_status` — filtered queries by status
- `idx_newsletter_subscribers_confirm_token` — double opt-in lookups
- `idx_newsletter_campaigns_tenant_status` — filtered campaign listings
- `idx_newsletter_campaigns_scheduled` — scheduler worker queries
- `idx_newsletter_subscriptions_unique` — unique subscriber+segment pair

## API Surface

### GraphQL

**Queries:**
- `newsletterSubscriber(id, tenantId)` — get subscriber by ID
- `newsletterSubscribers(tenantId, page, perPage, status, search)` — list subscribers
- `newsletterCampaign(id, tenantId)` — get campaign by ID
- `newsletterCampaigns(tenantId, page, perPage, status)` — list campaigns

**Mutations:**
- `newsletterSubscribe(input, tenantId)` — subscribe new email
- `newsletterConfirmSubscription(id, tenantId)` — confirm pending subscription
- `newsletterUnsubscribe(id, tenantId)` — unsubscribe from newsletter
- `newsletterUpdateSubscriber(id, input, tenantId)` — update metadata
- `newsletterDeleteSubscriber(id, tenantId)` — delete subscriber
- `newsletterCreateCampaign(input, tenantId)` — create draft campaign
- `newsletterUpdateCampaign(id, input, tenantId)` — update draft campaign
- `newsletterScheduleCampaign(id, input, tenantId)` — schedule campaign
- `newsletterCancelCampaign(id, tenantId)` — cancel campaign
- `newsletterDeleteCampaign(id, tenantId)` — delete campaign

### Ports (from `rustok-newsletter-api`)

**`NewsletterContentProvider`** — trait for source modules:
- `source_slug()` — identifier (e.g., "blog", "forum")
- `fetch_content(request)` — fetch recent content items
- `has_content(tenant_id)` — check if content available

**`NewsletterSubscriberPort`** — typed port for subscriber management:
- `subscribe()`, `confirm()`, `unsubscribe()`, `status()`

**`NewsletterCampaignPort`** — typed port for campaign operations:
- `create_campaign()`, `schedule_campaign()`, `cancel_campaign()`, `fetch_campaign_content()`

## State Machines

### Subscriber Lifecycle

```
Pending ──[confirm]──→ Active ──[unsubscribe]──→ Unsubscribed
  │                       │                           │
  └──[unsubscribe]──→ Unsubscribed ←──[reactivate]────┘
                          │
              Active ──[suppress]──→ Suppressed ──[reactivate]──→ Active
```

Valid transitions:
- Pending → Active (confirmed)
- Pending → Unsubscribed (explicit cancel before confirm)
- Active → Unsubscribed (user unsubscribes)
- Active → Suppressed (bounce/complaint)
- Unsubscribed → Active (re-subscribe with new confirmation)
- Suppressed → Active (manual admin re-activation)

### Campaign Lifecycle

```
Draft ──[schedule]──→ Scheduled ──[start_sending]──→ Sending ──[complete]──→ Sent
  │                       │                            │
  └──[cancel]──→ Cancelled ←──[cancel]─────────────────┘
```

Valid transitions:
- Draft → Scheduled (admin schedules for delivery)
- Draft → Cancelled (admin discards draft)
- Scheduled → Sending (scheduler picks up campaign)
- Scheduled → Cancelled (admin cancels scheduled campaign)
- Sending → Sent (delivery completes)
- Sending → Cancelled (admin aborts in-flight campaign)

## Module Layout

```
src/
  lib.rs              crate facade, re-exports
  module.rs           RusToKModule, MigrationSource, runtime registration
  error.rs            domain error contract
  domain/             state machines, value objects, invariants
  dto/                request/response data contracts
  entities/           SeaORM entity mappings
  services/           application use cases
  ports/              cross-boundary port implementations
  migrations/         schema migrations
  graphql/            GraphQL adapters
```

## FFA/FBA Readiness

### Status Block

- **Module slug:** `newsletter`
- **Ownership:** first_party
- **Runtime kind:** Optional
- **Rollback unit:** Component
- **Data boundary owner:** `newsletter_subscribers`, `newsletter_campaigns`, `newsletter_subscriptions`
- **Native migrations:** `m20261009_000040_create_newsletter_tables`
- **Supported migration policy:** AdditiveOnly
- **Predecessor standby strategy:** none
- **Rollback eligibility:** AutomaticSingleAttempt
- **Recovery invariants:** Monotonic epoch, idempotency receipts, zero-flapping

### Canonical Contracts

- **Domain errors:** `NewsletterError` with typed variants
- **Request context:** `PortContext` from `rustok-api`
- **Cross-module ports:** `NewsletterContentProvider`, `NewsletterSubscriberPort`, `NewsletterCampaignPort`
- **Persistence:** SeaORM entities, private to module
- **Lifecycle commands:** Execute domain transition table (subscriber/campaign state machines)
- **Public transports:** GraphQL with owner-safe error mapping

## Implementation Status

✅ **Completed:**
- API crate with contracts
- Domain state machines
- DTO layer
- Entity mappings
- Migrations
- Services (subscriber, campaign)
- Ports implementation
- GraphQL layer
- Module registration
- Workspace integration

⏳ **Next Steps:**
- Content provider implementations (blog, forum, commerce)
- Email template system integration
- Campaign scheduler worker
- Admin UI package (`rustok-newsletter-admin`)
- Integration tests
- FBA registry entry

## License

Workspace license applies.
