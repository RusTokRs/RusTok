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

## Acknowledged local contract listeners (2026-11-01)

The explicit persistent cursor remains mandatory for **remote** consumption. Local module
listeners now also opt into sealed payloads through the existing `EventHandler` / module
listener registry. This extends the canonical delivery bus, not a second event bus, untyped
channel or domain-specific host publisher. Established root APIs and wire formats stay intact.

`EventDispatcher::start_with_contract_delivery` binds the registered handlers to the transport's
**listener** bus. Local sealed delivery validates the exact original envelope, bounds in-flight
work, awaits all matching handlers, and propagates refusal/failure. No matching active listener
is a delivery error, not a successful broadcast with no receiver. Handler completion is the
acknowledgement boundary; the existing outbox row and relay retry/DLQ policy remain the durable
work and recovery mechanism. There is no additional in-memory queue or best-effort skip path.
The binding closes with its dispatcher; a stopped/panicked dispatcher cannot acknowledge facts.
Consumers must be idempotent under duplicates and refetch authoritative state under reordering.

The `outbox` Memory target uses that acknowledged path. For `outbox_iggy`, the existing configured
Iggy connector accepts the unchanged sealed envelope first, then matching local listeners must
complete before the relay can mark it delivered. A local failure can therefore republish the
same envelope identity to Iggy on retry: this is explicit at-least-once delivery, not a new fact.
An active listener registry is required before this decision; remote-only families with no
matching local handler still use their explicit persistent consumer groups. There is no
fallback from Iggy to Memory and no second bundled connector. The artifact delivery wrapper materializes matching subscriptions through the existing durable
binding queue with the sealed payload wrapper and original envelope identity. Cache wrappers
forward bounded families intact and retain their root-specific policy processing.

The Inventory reservation fact below is the concrete local consumer requirement. Its Search
listener must finish its Search-owned transaction before success; it does not certify generic
Index, external consumers, operational lag repair or broker restart behavior. Digest generation,
compilation and backend/runtime evidence are independent gates, never implied by source wiring.

## Inventory source delivery continuation (2026-11-02; source only)

The additive `InventorySourceEvent` family has `Changed` (baseline/Variant/positive owner
version) and `BaselineActivated` (baseline) facts. The canonical current-context owner writer
uses retained change UUID / baseline UUID as the existing write-once envelope identity; no
Product refresh proxy, second queue or observation-based replay is introduced. The compiled
sealed payload owns the mandatory module-delivery policy, not host domain-name matching:
Inventory source and reservation facts require acknowledged module listeners before relay
success in memory/Iggy fan-out. Unrelated remote-consumer families retain their cursor policy.
Schema registration is additive; canonical Rust digest generation/admission remains unrun.

## Inventory selected consumer composition (2026-11-02)

The existing Index source-factory catalog can contribute a real module event listener at the
normal listener lifecycle, after shared source capabilities exist. The selected Distribution
Inventory bridge uses that path for the two existing sealed source facts. It authenticates
current deployment-bound service credentials independently of envelope actors, persists loaded
current mutations before changed ACK and durably enqueues original-identity initial replay
before activation ACK. No root-event conversion, successful no-op handler, parallel bus or
new event version is introduced. An old delivery UUID is not reused for a changing loaded
projection payload. Core exposes the canonical sealed envelope/payload types used by its
public handler trait; Inventory re-exports its own existing source wire fact.
