# 2026-10-09: Newsletter Module Architecture

## Status

Accepted

## Context

The platform needed a newsletter/email campaign system that could aggregate content from multiple domains (blog, forum, commerce) and deliver email campaigns to subscriber lists. The key architectural question was: where should this functionality live?

### Options Considered

1. **Sub-feature of rustok-blog** — Newsletter as a blog extension
2. **Extension of rustok-notifications** — Merge with in-app notifications
3. **Extension of rustok-email** — Add campaign logic to transport layer
4. **Standalone module** — Separate bounded context

## Decision

**Newsletter is a standalone bounded context**, not a sub-feature of any source module.

### Rationale

1. **Cross-domain nature**: Newsletter aggregates content from blog, forum, AND commerce. Tying it to any single domain creates coupling and limits flexibility.

2. **Different bounded context from notifications**:
   - `rustok-notifications` = in-app notifications (inbox, fan-out, preferences)
   - `rustok-newsletter` = email campaigns (subscribers, campaigns, delivery)
   - Different data models, different delivery semantics, different user interactions

3. **Different bounded context from email**:
   - `rustok-email` = transport layer (SMTP, templates, delivery)
   - `rustok-newsletter` = business logic (who gets what, when, with what content)
   - Separation of concerns: newsletter uses email as transport

4. **Follows established pattern**: Mirrors the `rustok-notifications-api` / `rustok-notifications` split with `rustok-newsletter-api` / `rustok-newsletter`.

## Architecture

### Crates

- **`rustok-newsletter-api`** — Cross-boundary contracts
  - `NewsletterContentProvider` trait (for blog/forum/commerce to implement)
  - `NewsletterSubscriberPort` (typed port for subscriber management)
  - `NewsletterCampaignPort` (typed port for campaign operations)
  - Shared types: `ContentSourceSlug`, `SubscriberStatus`, `CampaignStatus`

- **`rustok-newsletter`** — Implementation
  - Domain: state machines, value objects
  - Services: subscriber management, campaign lifecycle
  - Ports: port implementations
  - GraphQL: admin API
  - Migrations: 3 tables

### Integration Pattern

Source modules integrate through **typed ports**, not direct dependencies:

```
rustok-newsletter-api  ← contracts
       ↑
rustok-newsletter      ← implementation
       ↑
rustok-blog            ← implements NewsletterContentProvider
rustok-forum           ← implements NewsletterContentProvider
rustok-commerce        ← implements NewsletterContentProvider
```

This achieves:
- **Inversion of control**: Newsletter doesn't know about blog internals
- **Testability**: Mock providers in tests
- **Extensibility**: New content sources add providers without changing newsletter
- **Clean dependencies**: No circular dependencies

### Database Schema

Three tables, all tenant-scoped:

1. **`newsletter_subscribers`**
   - Email, name, status (pending/active/unsubscribed/suppressed)
   - Confirmation tokens for double opt-in
   - Locale preference

2. **`newsletter_campaigns`**
   - Title, subject, preheader
   - Status (draft/scheduled/sending/sent/cancelled)
   - Content sources (JSON array of provider slugs)
   - Scheduled/sent timestamps

3. **`newsletter_subscriptions`**
   - Many-to-many: subscriber ↔ segment
   - Enables targeted campaigns

### State Machines

**Subscriber lifecycle:**
```
Pending ──[confirm]──→ Active ──[unsubscribe]──→ Unsubscribed
  │                       │
  └──[unsubscribe]──→ Unsubscribed
                          │
              Active ──[suppress]──→ Suppressed
```

**Campaign lifecycle:**
```
Draft ──[schedule]──→ Scheduled ──[send]──→ Sending ──[complete]──→ Sent
  │                       │                    │
  └──[cancel]──→ Cancelled ←──[cancel]─────────┘
```

## Consequences

### Positive

- **Clean separation of concerns**: Newsletter owns campaigns, email owns transport, sources own content
- **Extensible**: New content sources add providers without changing newsletter
- **Testable**: Mock providers for unit tests
- **Follows platform patterns**: Consistent with notifications architecture
- **Future-proof**: Can support additional delivery channels (SMS, push) without restructuring

### Negative

- **More crates**: Two additional crates (api + implementation)
- **Integration complexity**: Source modules must implement provider trait
- **Learning curve**: Developers must understand the provider pattern

### Neutral

- **Standalone module**: Requires explicit opt-in via `modules.toml`
- **Optional dependency**: Platform works without newsletter enabled

## Implementation Status

✅ **Phase 1 Complete:**
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

⏳ **Next Phases:**
- Content provider implementations (blog, forum, commerce)
- Email template integration
- Campaign scheduler worker
- Admin UI package
- Public subscription endpoints

## Related Decisions

- `2026-09-18-canonical-native-module-source-layout.md` — Module layout standard
- `2026-09-18-canonical-native-module-reference-contract.md` — Module contract standard
- Notifications module architecture (established the api/impl split pattern)

## References

- Module authoring guide: `docs/modules/module-authoring.md`
- Backend architecture: `docs/backend/module-backend-architecture.md`
- FFA/FBA unified plan: `docs/research/fluid-backend-architecture-unified-plan.md`
