# ADR: Event schema release discipline

- Status: accepted
- Date: 2026-07-23

## Context

RusToK event payloads are durable transport contracts. A Rust field, serde
attribute, registry entry, or envelope change can silently invalidate an
outbox row or a remote consumer even when local compilation succeeds. The
platform now generates valid JSON Schema from its Rust wire types, but it still
needed an executable release boundary around those generated schemas.

The current runtime has no approved inbound cross-replica consumer contract:
it does not yet own durable offsets, acknowledgement, replay, gap recovery, or
DLQ behavior for remote consumers. Claiming that a schema-version bump alone
provides backward compatibility would therefore be misleading.

## Decision

`rustok-events` computes deterministic SHA-256 digests for the sorted complete
event registry and for the root and typed transport wire schemas. The committed
`crates/rustok-events/contracts/event-contract-digests.json` artifact is checked
by the public contract test suite. Any wire or metadata drift fails the test and
requires an intentional artifact update during review.

All current published event schemas remain version `1`. A breaking change must
add a new event type rather than mutate an established event. Version `2` or a
dual-reader migration is prohibited until a new ADR defines the remote-consumer
delivery contract and the affected release plan names the migration owner,
supported reader versions, replay/offset behavior, and retirement condition.

## Consequences

- Schemars-generated wire schemas are protected against accidental changes.
- Registry descriptions and field metadata receive the same release review as
  serde payload shape.
- The platform does not promise compatibility it cannot yet deliver.
- A later inbound consumer design can relax the version-1 gate only together
  with executable migration and recovery evidence.
