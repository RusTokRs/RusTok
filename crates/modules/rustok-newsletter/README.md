# rustok-newsletter

Newsletter subscriber management, campaign lifecycle, and email delivery orchestration for the RusToK platform.

## Purpose

This module provides a cross-domain newsletter system that aggregates content from
multiple source modules (blog, forum, commerce) and delivers email campaigns to
subscriber lists.

## Architecture

### Bounded Context

Newsletter is a standalone bounded context, not a sub-feature of any source module:

- **Subscriber management** — opt-in/opt-out, double opt-in, suppression
- **Campaign lifecycle** — draft → scheduled → sending → sent / cancelled
- **Content aggregation** — source modules implement `NewsletterContentProvider`
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

### Dependencies

- `rustok-email` — transport (`EmailDeliveryPort`)
- `rustok-outbox` — transactional event publishing
- `rustok-newsletter-api` — cross-boundary contracts

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

## License

Workspace license applies.
