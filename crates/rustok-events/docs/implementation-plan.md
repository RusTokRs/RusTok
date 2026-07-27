# Implementation plan for `rustok-events`

## Source of truth

This file is the canonical live plan for shared event contracts. It does not own
transport implementations, but it records the runtime guarantees that consumers
may rely on and the remaining cross-replica delivery dependencies.

- `[x]` means the source contract is present in the current owner slice and protected
  by a test or architecture guard.
- `[ ]` means implementation or verification is still required.
- Transport-specific implementation remains with the platform runtime, Iggy,
  outbox, or the consuming owner module.

Last reconciled with `main`: 2026-07-27.

## Current state

`rustok-events` is the canonical source of `DomainEvent`, `EventEnvelope`,
schema metadata, validation, and event versioning policy. `rustok-core::events`
is a compatibility re-export only; domain, outbox, runtime, and test crates
should import event contracts from this module.

The committed `contracts/event-contract-digests.json` release artifact gates the
full schema registry and all root/typed transport wire schemas. Contract tests
regenerate its values with Schemars and fail on drift. The combined Translation and
Social Graph artifact was regenerated with the deterministic
`event_contract_digests --write` example and is committed on this branch. Tests
and builds never update it implicitly. The current release
train intentionally allows only version-1 event schemas: a versioned payload
migration remains blocked until the remote-consumer delivery contract is owned.

The root schema registry covers every current root event type. Schemars now
generates standards-compliant Draft 2020-12 JSON Schema for the root event and
envelope wire representations, while `jsonschema` validates those artifacts in
the contract test suite. Root envelopes validate their metadata, registered
schema, and semantic payload at the event bus, outbox write, outbox relay, and
JSON/MessagePack decode boundaries. The server outbound event bus now has atomic
context registration, abort-on-drop ownership, restart after panic or unexpected
exit, and critical readiness escalation when the supervisor stops. The configured
`EventRuntime` is published before module dispatcher startup.

The root `module.effective_policy_revision_changed` event is the canonical
predecessor-bound producer contract for effective-policy projections; it is
validated as a digest transition and is appended only through an owner transaction
boundary. The root `build.rolled_back` event is likewise explicit: it carries the
requested/restored builds and source/target releases, while the envelope carries
the actor. Schema-registry exact-set coverage now also includes the registered
module security/distribution events and the previously missing comment schemas.
The root user-registration fact is `user.account_registered` v1 and contains only
`user_id`; the former email-carrying payload was atomically removed because the
repository has no production publisher or reader for it.

The root `translation.target.changed` v1 fact is content-free and carries only
owner/resource identity, changed exact locale, opaque revisions, operation, and
correlation. Owner providers publish it transactionally with their normal
localized write and idempotency receipt.

The typed-family implementation now includes sealed version-1
`social_graph.relation.state_changed`. Its payload is relation-only: relation id,
source/target user ids, canonical relation kind, active state, and revision.
Tenant and actor remain envelope metadata. Command idempotency, expected revision,
request context, receipt snapshots, claims, roles, locale, and channel are
excluded. `rustok-social-graph` maps this family and publishes it through the
existing transactional outbox seam; arbitrary external event names remain
impossible because `EventContract` stays sealed in this crate. The combined digest artifact is committed; remaining release-contract work
is approved consumer delivery and recovery ownership.

The Social Graph owner also provides bounded historical replay over the same
sealed persisted-revision fact. Replay is service/system-only, tenant-scoped,
UUID-cursor bounded, dry-run capable, and page-atomic in Outbox. It begins only
after event-aware writers are active, so the cursor covers the fixed historical
backlog while concurrent new relations use the live atomic path. This owner
primitive does not define a consumer projection. A future approved consumer must
persist by relation id plus monotonic revision, ignore duplicate or lower
revisions, and acknowledge only after its own result is durable.

The module-build dispatcher is the first owner-specific remote consumer: it
retains one remote Iggy cursor, persists or recognizes an idempotent owner result,
and only then commits the broker offset. Module listeners still receive the
configured local listener bus; an owner that requires durable cache or projection
recovery must use a persisted outbox/stream offset or monotonic generation rather
than assuming remote event replay.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core -> transport -> ui/leptos`, with a sibling module-owned
  Next package.
- `rustok-events-module` is the cycle-free runtime/manifest adapter for this
  contract crate. Its `admin/` package owns the delivery-profile UI and uses
  native `#[server]` functions by default with a parallel GraphQL adapter.
- The host owns only route composition and provides `SharedEventDeliveryControl`;
  it does not own event-profile fields or UI.
- Next ownership is under `crates/rustok-events-module/next-admin`; the host
  consumes it as `@rustok/events-admin`.

## Completed source results

- [x] Keep one canonical event/envelope/schema definition in `rustok-events`.
- [x] Validate root and typed-family payloads at publication, durable relay, and streaming decode boundaries.
- [x] Keep the root event registry synchronized with all current root event types.
- [x] Generate and validate standards-compliant root event/envelope JSON Schema from Rust types.
- [x] Gate registry and root/typed transport schema drift with a committed release artifact.
- [x] Provide a deterministic generator for reviewed release-artifact updates.
- [x] Regenerate and commit the combined Translation/Social Graph digest artifact.
- [x] Block unplanned version-2 schemas until durable remote-consumer migration ownership exists.
- [x] Keep contact data out of shared user-registration event payloads.
- [x] Keep `translation.target.changed` content-free and transactionally owner-published.
- [x] Own the server outbound forwarder through a context runtime handle.
- [x] Restart the outbound forwarder after panic or unexpected exit.
- [x] Surface a terminal forwarder as a critical runtime guardrail condition.
- [x] Publish the configured runtime/listener bus before module dispatcher startup.
- [x] Add a permanent path-scoped event-runtime lifecycle gate.
- [x] Keep explicit platform rollback facts synchronized across the owner event,
  root schema registry, and transport adapters.
- [x] Add the sealed `social_graph.relation.state_changed` v1 implementation with
  registry metadata, semantic validation, safe-field tests, and direct owner
  publication through `TransactionalEventBus`.
- [x] Add owner-bounded replay of persisted Social Graph relation revisions through
  the same sealed family, with service/system policy, tenant UUID cursor, dry-run,
  page-atomic rollback, and source guardrails.

## Open results

1. **Keep the reviewed event-contract digest synchronized.** The combined
   Translation/Social Graph artifact is committed. Regenerate it only with the
   deterministic example when a reviewed contract shape changes.
   **Depends on:** the change-owning event family and schema review.
   **Done when:** `contracts/event-contract-digests.json` equals
   `event_contract_digests()` and canonical contract tests report no drift.

2. **Keep event types, schema registry, release artifact, and consumer imports synchronized.**
   Add an event family only with canonical schema/validation coverage and direct
   consumer imports from `rustok-events`.
   **Depends on:** the change-owning domain module and its outbox path.
   **Done when:** the event, registry metadata, committed digest artifact,
   consumer imports, and contract tests describe the same payload and tenant
   behavior.

3. **Provide an approved inbound delivery contract for remote consumers.** The
   local listener bus is not replayable and does not consume remote Iggy/outbox
   deliveries. Define which platform component owns receive, acknowledgement,
   persisted offsets, restart, gap recovery and DLQ behavior before owner modules
   use events for cross-replica cache or projection correctness.
   **Depends on:** the selected Iggy/outbox runtime and an explicit consumer
   group/offset contract.
   **Done when:** at least one multi-replica owner consumer can miss a fast-path
   event, restart, replay from persisted state, recover the affected projection
   or cache, and acknowledge only after successful application. The module-build
   dispatcher supplies the result-first cursor shape; real-broker multi-replica
   recovery evidence remains outstanding.

4. **Define approved consumers for Social Graph relation events.** Do not create a
   projection merely because the event and bounded owner replay exist. Name a
   concrete consumer requirement, define its idempotent durable result and
   acknowledgement boundary, and keep Social Graph relation persistence as the
   drift-repair authority.
   **Depends on:** a real consumer requirement and the inbound persisted-offset
   contract.
   **Done when:** a bounded consumer uses the delivered owner replay or rescan,
   ignores duplicate/lower revisions, repairs drift against Social Graph, persists
   its owner-specific result, and acknowledges only after that result is durable.

5. **Synchronize event contracts with recovery guidance.** Update outbox, replay,
   reindex, and DLQ documentation with a schema or versioning change.
   **Depends on:** the relevant runtime/operational contract.
   **Done when:** recovery procedures name the correct event schema and do not
   rely on transport-owned copies of event payloads.

## Verification

Contract tests cover public event-contract use cases.

- `cargo xtask module validate events`
- `cargo xtask module test events`
- `cargo test -p rustok-events --test social_graph_contracts -- --nocapture`
- `cargo run -p rustok-events --example event_contract_digests`
- `cargo test -p rustok-social-graph --test relation_event_replay_sqlite -- --nocapture`
- `node scripts/verify/verify-social-graph-relation-event-replay.mjs`
- `cargo test -p rustok-server --test event_bus_runtime_guard`
- `cargo test -p rustok-server event_forwarder --lib`
- `cargo clippy -p rustok-server --lib -- -D warnings`
- Targeted schema coverage, validation, compatibility-alias, envelope JSON
  roundtrip, inbound replay and multi-replica recovery tests.

## Change rules

1. Keep canonical event payloads and schemas in this module.
2. Keep transport-specific execution in its runtime owner; do not copy event
   payload definitions into transport crates.
3. Update the committed digest artifact only through an intentional contract
   review; never weaken or bypass the canonical digest comparison.
4. Update local docs, `rustok-module.toml`, event-flow documentation, and
   outbox/replay guidance with a contract change.
5. Update `docs/modules/implementation-plans-registry.md` only for status and
   nearest priority.