# Forum topic list default ordering uses the tenant setting

- Date: 2026-10-28
- Decision status: Accepted
- Implementation status: Implemented
- Owners: rustok-forum
- Extends: [Forum list page-size defaults](./2026-10-09-forum-list-page-size-from-settings.md)
- Supersedes: None
- Superseded by: None

## Context

The canonical list filter supports latest-reply keyset ordering and top-score keyset ordering. Tenants need configurable default topic list ordering.

## Decision

The tenant settings owner reads `default_topic_sort` through the existing static settings port. It accepts `latest_reply` (default, maps to `Latest`) and `top` (maps to `Top`).

## Sources of truth and ownership

`rustok-forum` owns topic ordering settings and list filter resolution.

## Invariants

### Allowed states
- `default_topic_sort` set to `latest_reply` or `top`.

### Forbidden states
- Silently remapping or destroying unsupported sort options; typed error is returned.

## Non-goals

- Implementing `created_at`, `votes`, or `views` sort orders.

## Data, transaction, and concurrency boundary

Not applicable. Reads static settings; no schema or table migrations.

## Context dimensions

- Tenant: Read per tenant via static settings port.

## Events and projections

No new events or projections.

## Failure semantics

Settings port failure propagates typed error; missing setting falls back to `latest_reply`.

## Migration and cutover

Direct cutover. Omitted order parameter in REST and GraphQL applies tenant default.

## Alternatives considered

1. **Hardcoded server-wide default:** Rejected to allow per-tenant engagement modes.

## Verification

Unit tests for default, top, invalid, and case-sensitive values.

## Consequences

Enables tenant-configured default topic ordering while preserving explicit caller override.
