# ADR: Sealed typed event families

- Status: accepted
- Date: 2026-07-17

## Context

The original event contract stores every platform event in one `DomainEvent`
enum. Adding each bounded-module lifecycle to that enum creates central merge
pressure, forces unrelated domains to release together, and makes independent
schema evolution harder. Replacing it with arbitrary event-type strings and raw
JSON would remove compile-time safety and allow unregistered payloads into the
transactional outbox.

Marketplace listing needs nine external lifecycle events with an intentionally
small payload. Its immutable owner timeline also contains moderation prose and
legacy snapshots that must never be relayed as live external commands.

The original outbox relay and Iggy APIs accepted only `EventEnvelope<DomainEvent>`.
Persisting a separate JSON payload shape without updating relay, streaming, and
consumer boundaries would cause valid bounded-family events to be retried and
moved to the DLQ.

## Decision

`rustok-events` owns a sealed `EventContract` trait. Implementations can only be
added inside that crate. Established root events continue to implement the
contract, while bounded modules may receive a dedicated typed enum with explicit
variants, validation, schema lookup, and versioning.

`ContractEventPayload` is the transport-level typed family wrapper. It adds one
platform variant per bounded event family rather than one variant per lifecycle
event. `ContractEventEnvelope` validates the concrete event, registered schema,
and duplicated envelope metadata before durable persistence or transport.

`rustok-outbox` exposes
`TransactionalEventBus::publish_contract_in_tx`; it accepts a sealed typed
contract and a live owner transaction. The relay supports both established root
`EventEnvelope` records and new `ContractEventEnvelope` records, validates the
row/envelope metadata pair, and dispatches bounded families through
`EventTransport::publish_contract`.

Streaming transports serialize the typed contract envelope in the configured
format. Iggy supports both JSON and Postcard and provides an explicit persistent
contract consumer cursor. Existing root consumers are not given a silent
fallback that could reinterpret a bounded-family event as `DomainEvent`.

`MarketplaceListingEvent` is the first bounded-module family. It contains nine
explicit version-1 variants. External payloads include only listing identity,
seller/product references, market/channel scope, and terms version. Notes,
reasons, arbitrary metadata, and legacy snapshot provenance remain owner-private.

## Consequences

- bounded event families evolve without enlarging one platform-wide lifecycle enum;
- arbitrary event names and unregistered payloads remain impossible;
- outbox persistence, relay, streaming, and consumption preserve typed families;
- JSON and Postcard remain supported transport profiles;
- existing `DomainEvent` publishers and consumers remain source-compatible;
- bounded-family consumers must opt into the explicit contract consumer cursor;
- consumers use canonical event-type and schema registries from `rustok-events`;
- owner modules still must publish inside the same database transaction as state,
  timeline, and durable receipt changes.
