# Global event delivery profiles

- Date: 2026-07-23
- Status: Accepted

## Context

The old event configuration exposed independent transport, relay-target, and
fallback switches through tenant-scoped platform settings. That made a global
runtime topology appear tenant-configurable, allowed impossible combinations,
and could suggest that Iggy was optional even when selected.

## Decision

RusToK has exactly two global event-delivery profiles:

- `outbox` for transactional single-node delivery and the default
  profile;
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

- A lightweight production installation can use `outbox` without Iggy.
- Local development and test environments use `outbox`; no fallback path
  is part of the platform.
- Production Iggy failures are explicit at startup instead of silently changing
  delivery semantics.
- Operators must include a controlled restart after profile changes.
- Tenant settings APIs cannot mutate global event delivery.

## Durable fanout recovery (source only)

The relay pins a versioned delivery route per event before invoking any target
or wrapper. An incompatible route refuses delivery before external I/O; review
and drain pending/DLQ events before changing profiles or broker routing.
The server external-Iggy fanout records primary acceptance in a separate
short SQL transaction under the exact live claim and open dispatch intent.
On retry, a committed checkpoint skips the primary call and retries local
delivery. A crash between broker acceptance and checkpoint commit is still
ambiguous and can redeliver. The original event UUID remains the broker ID.
The same route also pins whether local delivery was required when fanout
first reached its validated registry. A mismatch fails closed.

Local handlers, cache rotations and other process-local effects are not
skipped on the basis of another host's success. Bundled Iggy is deliberately
not eligible for primary skipping because the configured local path is not
proof of shared durable broker identity. External configuration fingerprint
covers credential-free endpoint routing, protocol/TLS domain, stream and
serialization, not remote broker identity or retention. Replacing a broker
behind the same endpoints requires reviewed recovery; never reuse a route
checkpoint against a different broker. The standalone broker-only relay has
a distinct route from the server fanout; do not run different route owners
against the same pending queue. Generic library targets use an unclassified
route unless the host explicitly supplies a stable one.

The route and checkpoint share the event retention lifecycle, including DLQ
replay; replay does not silently discard confirmed evidence. Old relay
binaries must be stopped/drained before migration
`m20261127_000205_create_outbox_delivery_checkpoints`. No SQL transaction
spans broker I/O. This is not an exactly-once guarantee, a complete
per-handler receipt system, or a proof that the broker retained an accepted
message. Source regression tests are authored, not executed.

A changed bound route is now classified as a pre-publication validation
failure through the normal bounded retry/backoff and DLQ policy, releasing
the exact claim rather than monopolizing capacity until lease expiry.
Checkpoint-enabled primary publication refuses without its relay task scope;
the test-only uncheckpointed transport retains ordinary direct behavior.
The server regression exercises local failure, relay reconstruction and
confirmed-primary skipping with a transport double (not a live broker).
