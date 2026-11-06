# Newsletter Module Architecture

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: Implemented
- Owners: rustok-newsletter
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

The platform requires an email campaign and newsletter capability that can aggregate content from multiple domains (such as `rustok-blog`, `rustok-forum`, and `rustok-commerce`) and deliver personalized email campaigns to subscriber lists.

Key architectural challenges:
1. Subscribing to newsletters and tracking delivery lifecycles requires distinct bounded context from generic in-app notifications (`rustok-notifications`) and low-level email transport (`rustok-email`).
2. Tying campaign logic to a single source domain (e.g. `rustok-blog`) creates tight coupling and prevents reusing the campaign engine for product announcements, forum digests, or transactional re-engagement.
3. Content aggregation across multiple domains requires an inversion-of-control provider seam rather than newsletter hardcoding dependencies on specific domain crates.

## Decision

**Adopt `rustok-newsletter` as a standalone bounded context and platform module** with an API-decoupled provider pattern:

1. **Standalone Bounded Context:**
   - `rustok-newsletter` manages subscriber records, double opt-in confirmation tokens, subscription segments, campaign lifecycle, and dispatch scheduling.
   - `rustok-email` serves strictly as the underlying delivery transport (SMTP/SES/templates).
   - `rustok-notifications` continues to own in-app user notifications, inbox states, and fan-out, completely decoupled from email campaigns.

2. **Inversion of Control via Typed Provider Ports:**
   - `NewsletterContentProvider` trait allows source modules (`rustok-blog`, `rustok-forum`, `rustok-commerce`) to export content feeds into campaigns without `rustok-newsletter` depending on source crate internals.
   - Subscribers and campaigns are managed via typed ports (`NewsletterSubscriberPort`, `NewsletterCampaignPort`).

## Sources of truth and ownership

- **Canonical Owner:** `crates/modules/rustok-newsletter` owns all campaign entities and subscriber states.
- **Authoritative Database Tables:**
  - `newsletter_subscribers`: Tenant-scoped subscriber identities, emails, confirmation tokens, and locale preferences.
  - `newsletter_campaigns`: Tenant-scoped campaign definitions, status, subjects, preheaders, and referenced content provider slugs.
  - `newsletter_subscriptions`: Association between subscribers and target newsletter topics/segments.
- **Transport & Projections:**
  - Admin management endpoints exposed via GraphQL and REST routers.
  - Public subscription and double opt-in confirmation endpoints exposed through owner HTTP router.

## Invariants

### Allowed states

- **Subscriber Lifecycle:**
  `Pending` (awaiting double opt-in confirmation) $\to$ `Active` (confirmed) $\to$ `Unsubscribed` (user opted out) or `Suppressed` (bounce/complaint).
- **Campaign Lifecycle:**
  `Draft` $\to$ `Scheduled` $\to$ `Sending` $\to$ `Sent` or `Cancelled`.

### Forbidden states

- A subscriber record cannot exist without a valid `tenant_id` and normalized email.
- Unconfirmed (`Pending`) or `Unsubscribed`/`Suppressed` recipients must never receive campaign broadcasts.
- A campaign in `Sending` or `Sent` status cannot be modified or re-scheduled.

## Non-goals

- Low-level SMTP protocol handling (delegated strictly to `rustok-email`).
- Direct rendering of blog/product HTML templates inside the newsletter engine (content is provided structured through `NewsletterContentProvider`).
- In-app notification inbox and web push (owned by `rustok-notifications`).

## Data, transaction, and concurrency boundary

- All mutations (`newsletter_subscribers`, `newsletter_campaigns`, `newsletter_subscriptions`) run within an explicit SeaORM database transaction per tenant.
- Double opt-in confirmation tokens are cryptographically secure, single-use, and expire after a configurable duration.
- Campaign dispatch is idempotent: dispatch workers process batches with transactional state locks to avoid duplicate email transmissions.

## Context dimensions

- **Tenant Isolation:** Every query and write enforces `tenant_id` filtering; cross-tenant campaigns or list sharing are strictly prohibited.
- **Locale Awareness:** Subscriber language preference is recorded at subscription time and used to query localized content variants from providers.

## Events and projections

- Emits `NewsletterSubscribed`, `NewsletterConfirmed`, and `NewsletterUnsubscribed` domain events into the transactional outbox (`sys_events`).
- Emits `NewsletterCampaignScheduled` and `NewsletterCampaignCompleted` for audit logs and analytics.

## Failure semantics

- Invalid email formatting or duplicate active subscription attempts return typed `NewsletterError::InvalidEmail` or `NewsletterError::AlreadySubscribed`.
- Failed delivery reports from `rustok-email` trigger automatic subscriber suppression (`NewsletterSubscriberStatus::Suppressed`) to maintain sender reputation.

## Migration and cutover

- Migration scripts create `newsletter_subscribers`, `newsletter_campaigns`, and `newsletter_subscriptions` with appropriate PostgreSQL/MySQL/SQLite indexes.
- No legacy data cutover needed for new standalone module installations.

## Alternatives considered

1. *Embedding into `rustok-blog`:* Rejected because newsletter campaigns require multi-domain content aggregation (commerce, forum) and separate sender reputation management.
2. *Merging into `rustok-notifications`:* Rejected because in-app notifications have completely different data models, delivery semantics, and user interaction expectations.
3. *Single `rustok-email` crate with campaign logic:* Rejected to preserve clean transport vs business logic boundaries.

## Verification

- Crate unit tests: verify subscriber state machine transitions, double opt-in token generation, and campaign lifecycle validations.
- Mock content provider integration tests: verify cross-module content aggregation through `NewsletterContentProvider`.
- Boundary verification: verify zero direct dependency on `rustok-blog` or `rustok-commerce` inside `rustok-newsletter`.

## Consequences

### Positive

- Clean separation of concerns between campaign orchestration, email transport, and content generation.
- Extensible provider model: new content sources can be plugged in without touching newsletter code.
- Full multi-tenant isolation and double opt-in regulatory compliance.

### Negative

- Requires source modules to implement the content provider port to appear in automated digests.
