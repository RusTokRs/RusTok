# Global event delivery profiles

- Date: 2026-07-23
- Status: Accepted

## Context

The old event configuration exposed independent transport, relay-target, and
fallback switches through tenant-scoped platform settings. That made a global
runtime topology appear tenant-configurable, allowed impossible combinations,
and could suggest that Iggy was optional even when selected.

## Decision

RusToK has exactly three global event-delivery profiles:

- `memory` for process-local development and test delivery;
- `outbox_local` for transactional single-node production delivery;
- `outbox_iggy` for transactional Iggy-backed high-throughput or multi-process
  delivery.

The desired profile is stored in a singleton global table, not in tenant
`platform_settings`. Saving a profile never hot-swaps the running transport;
an operator performs a controlled restart to activate it. The API exposes both
the active and desired profile.

`outbox_iggy` is accepted only after server-side validation of the Iggy
deployment configuration. The UI presents a configuration dialog when that
precondition is absent, and the backend rejects a bypassed request. No profile
contains Iggy credentials or endpoints; those remain owned by the
`iggy_connector` capability configuration.
There is no direct-Iggy profile and no fallback from `outbox_iggy` to local
delivery.

## Consequences

- A lightweight production installation can use `outbox_local` without Iggy.
- Production Iggy failures are explicit at startup instead of silently changing
  delivery semantics.
- Operators must include a controlled restart after profile changes.
- Tenant settings APIs cannot mutate global event delivery.
