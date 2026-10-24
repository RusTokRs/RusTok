# rustok-newsletter / CRATE_API

## Public surface

- Modules: `domain`, `dto`, `entities`, `error`, `graphql`, `migrations`, `model`, `module`, `ports`, `provider`, `services`.
- Types: `NewsletterModule`, `SubscriberService`, `CampaignService`, `SubscriberPortImpl`, `CampaignPortImpl`, `ContentProviderRegistry`, `NewsletterError`, `NewsletterResult<T>`.
- Core DTOs & Models: `SubscriberStatus`, `CampaignStatus`, `SubscribeInput`, `CreateCampaignInput`, `ScheduleCampaignInput`, `NewsletterContentItem`, `ContentFetchRequest`.

## Platform Capability Bindings
- **Core Module Registration**: Exposes `NewsletterModule` implementing `RusToKModule` (slug `"newsletter"`, kind `ModuleKind::Optional`, dependencies `["email", "outbox"]`) and `MigrationSource`.
- **Subscriber & List Management**:
  - `SubscriberService`: Double opt-in confirmation tokens, subscription lists, status transitions (`Pending`, `Active`, `Unsubscribed`, `Suppressed`).
  - `NewsletterSubscriberPort`: Neutral port for subscribing, unsubscribing, and status inspection.
- **Campaign Lifecycle & Delivery**:
  - `CampaignService`: Campaign creation, content assembly, scheduling, dispatch, cancellation, and metrics.
  - `NewsletterCampaignPort`: Cross-module campaign scheduling and management.
- **Content Aggregation**:
  - `ContentProviderRegistry` / `NewsletterContentProvider`: Dynamic digest content aggregator for Blog, Forum, and Products.

## Domain Collaborations
- **Core Platform**: Integrates with `rustok-core` module system and `rustok-api` port context and errors.
- **`rustok-email`**: Transports confirmation emails, unsubscribe notices, and batched newsletter deliveries.
- **`rustok-outbox`**: Emits transactional outbox events on subscriber status and campaign progression.

## Minimum Contract Set

### Input DTOs/Commands
- `SubscribeInput` (`email`, `name`, `locale`)
- `ConfirmSubscriptionInput` (`token`)
- `UnsubscribeInput` (`token`, `reason`)
- `CreateCampaignInput` (`title`, `subject`, `content_html`, `content_text`, `segment_id`)
- `ScheduleCampaignInput` (`scheduled_at`)

### Domain Invariants
- Campaign lifecycle transitions: Only `Draft` is editable; `Draft` -> `Scheduled` -> `Sending` -> `Sent`; `Draft`/`Scheduled` -> `Cancelled`. `Sent` and `Cancelled` are terminal.
- Subscriber lifecycle transitions: `Pending` -> `Active` -> `Unsubscribed` / `Suppressed`. Same state is not a valid transition.
- Tenant isolation: All subscriber, list, campaign, and dispatch records are strictly partitioned by `tenant_id`.
- Email format & uniqueness: Emails must be valid format and unique per tenant. Duplicate active subscriptions return conflict / idempotent existing ID.

### Events / Outbox Side Effects
- Subscription confirmation emits `SubscriberConfirmed` event through `rustok-outbox`.
- Unsubscription emits `SubscriberUnsubscribed` event through `rustok-outbox`.
- Campaign dispatch progression emits campaign lifecycle events through `rustok-outbox`.

### Errors / Failure Codes
- `NewsletterError::SubscriberNotFound` — subscriber ID or token does not exist.
- `NewsletterError::DuplicateSubscriber` — email is already subscribed in this tenant.
- `NewsletterError::InvalidConfirmationToken` — confirmation or unsubscribe token is expired/invalid.
- `NewsletterError::InvalidSubscriberState` — illegal subscriber state machine transition.
- `NewsletterError::CampaignNotFound` — campaign ID does not exist.
- `NewsletterError::InvalidCampaignState` — illegal campaign state machine transition.
- `NewsletterError::Database` — underlying SeaORM persistence error.
