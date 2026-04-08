# rustok-order

## Purpose

`rustok-order` is the default order submodule of the `Ecommerce` family.

## Responsibilities

- Own the order write-side schema, service, and status transitions.
- Persist order snapshots and line items independently from catalog ownership.
- Resolve order-owned Flex attached custom fields through the shared `flex`
  multilingual attached-value contract while preserving non-Flex operational
  metadata in `orders.metadata`.
- Publish transactional order lifecycle events through the outbox.
- Publish a module-owned Leptos admin UI package in `admin/` for order
  operations and lifecycle handling.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Depends on `flex` for shared attached localized-value storage helpers used by
  order custom-field multilingual flows.
- Depends on `rustok-events` and `rustok-outbox` for transactional domain-event publishing.
- Used by `rustok-commerce` as the default order submodule of the ecommerce family.
- Keeps product and variant references as snapshots so the order domain does not depend on
  the product module as a lower-level shared layer.
- `apps/admin` consumes `rustok-order-admin` through manifest-driven composition,
  while GraphQL/REST order transport remains in `rustok-commerce`.

## Entry points

- `OrderModule`
- `OrderService`
- `rustok-order-admin`
- `dto::*`
- `entities::*`

See also `docs/README.md`.
