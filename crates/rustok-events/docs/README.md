# Documentation `rustok-events`

`rustok-events` is the canonical shared import surface for platform event
contracts. It owns root events, sealed module event families, typed envelope
shapes, schema metadata, and validation rules.

## Purpose

- publish a unified event-contract layer for the platform;
- keep schema metadata, envelope shape, validation, and event-type versioning in one crate;
- allow bounded modules to publish typed event families without reopening a platform-wide lifecycle enum;
- prevent arbitrary string event names and unregistered payloads from reaching durable transports.

## Responsibilities

- `DomainEvent` and `EventEnvelope` for the established root event family;
- sealed `EventContract` implementations for module event families;
- `ContractEventPayload` as the typed family wrapper used by durable and streaming transports;
- `ContractEventEnvelope` with validation of payload, registered schema, and envelope metadata;
- `EventSchema`, `FieldSchema`, schema lookup, combined schema iteration, and
  Schemars-generated Draft 2020-12 JSON Schema for root events and envelopes;
- validation and versioning policy for every public event payload;
- transport-independent contracts only; persistence and delivery remain owned by `rustok-outbox` and streaming adapters.
- root envelopes validate registered schema metadata and semantic payload rules at
  every durable or remote ingress boundary;
- a committed digest artifact covers the complete registry plus root and typed
  transport wire schemas, making unreviewed schema drift fail contract tests;
- root static-distribution events expose immutable queue identity, lease-claim
  audit, terminal result digest, and verified release-head activation identity
  plus rebuild-only rollback and release revocation identity without carrying
  build logs, operator reasons, or evidence payloads inline.
- root effective-policy transition events carry an explicit consumer key and
  digest predecessor/successor pair so revision-aware projections never infer
  ordering from opaque hashes.
- root `build.rolled_back` events preserve requested/restored build identity
  and the exact release predecessor transition; actor identity remains in the
  canonical envelope.
- root `user.account_registered` events expose only a stable user identity;
  email addresses and other contact data remain in the auth/user owner.

## Sealed event-family contract

`EventContract` is sealed inside this crate. A domain module cannot implement it
locally and cannot publish an arbitrary `(event_type, payload)` pair. New event
families are defined and reviewed in `rustok-events`, then published through the
typed transactional API in `rustok-outbox`.

One `ContractEventPayload` variant is added per bounded event family, not per
lifecycle event. The family enum owns its lifecycle variants and schema evolution.
This keeps transport serialization typed while avoiding continuous growth of the
legacy platform-wide `DomainEvent` enum.

`MarketplaceListingEvent` is the first module family using this boundary. It
contains nine explicit versioned variants and exposes only stable listing
identity, seller/product references, market/channel scope, and terms version.
Moderation prose and arbitrary owner metadata remain private to the listing
timeline and are not part of the external contract.

## Schema release discipline

`contracts/event-contract-digests.json` is the committed release artifact for
the sorted complete registry, root `DomainEvent`/`EventEnvelope` wire schemas,
and typed `ContractEventPayload`/`ContractEventEnvelope` wire schemas. The
`published_event_contract_matches_committed_release_artifact` contract test
regenerates all of those values with Schemars and fails on any drift.
The contract suite also rejects direct contact, secret, rendered-body, and
arbitrary-metadata field names in every shared event schema; this guardrail is
an additional review aid, not a substitute for owner-level data classification.

During the current release train every published schema is version `1`. A
breaking payload change is not permitted merely by changing a Rust type or
updating the digest. It first needs an accepted ADR, a named consumer migration
owner, supported reader versions, and durable remote-consumer replay/offset
semantics. Until then, add a new event type for a new fact and leave an existing
payload unchanged.

Security removals may replace an unused initial event contract atomically when
the owning review confirms that no production publisher or external reader
exists. The `user.registered` payload was removed under that rule because it
carried email; its replacement is `user.account_registered` v1 with `user_id`
only.

## Integration

- `rustok-core::events` remains a compatibility adapter for the established root family;
- domain modules, outbox/runtime crates, and tests import canonical contracts from `rustok-events`;
- `rustok-outbox::TransactionalEventBus::publish_contract_in_tx` accepts only sealed typed contracts;
- the outbox relay supports both root and typed-family envelopes and validates row/envelope metadata;
- JSON and MessagePack root-envelope decoders reject malformed metadata, unregistered
  event types, and semantically invalid payloads before consumers receive them;
- streaming adapters preserve the configured JSON or MessagePack profile;
- bounded-family consumers use an explicit typed contract consumer cursor rather than a root-event fallback;
- breaking payload changes require the accepted migration contract described above.

## Verification

- `cargo xtask module validate events`
- `cargo xtask module test events`
- `node scripts/verify/verify-marketplace-listing-event-contract.mjs`
- targeted schema, validation, relay, streaming, consumer, and serialization tests

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Platform documentation map](../../../docs/index.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Sealed typed event-family ADR](../../../DECISIONS/2026-07-17-sealed-typed-event-families.md)
