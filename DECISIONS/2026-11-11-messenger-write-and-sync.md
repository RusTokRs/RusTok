# Messenger write authority and resumable delivery

- Date: 2026-11-11
- Decision status: Proposed
- Implementation status: Not started
- Owners: `rustok-messenger`
- Extends: [Outbox runtime](2026-03-11-queue-runtime-source-of-truth-outbox.md), [Port ownership](2026-07-01-port-contract-ownership-and-runtime-feature-boundary.md)
- Supersedes: None
- Superseded by: None

## Context

Messenger has a PostgreSQL-backed owner, several request adapters, and a process-local room hub. FastSync currently scans message IDs generated before commit. Edits and deletes change existing message rows, so scanning for larger IDs cannot replay them. Independently constructed services and hubs also prevent those objects from serving as shared sequence or delivery authorities.

## Decision

Keep one Messenger write owner in the embedded FBA profile and deploy the Connection Gateway as a separate owner-owned runtime from the first private release. Split the owner implementation by conversation and membership, message timeline, synchronization and read state, and realtime delivery. Expose a single owner application boundary through native, GraphQL, HTTP, and cross-module adapters. The gateway uses a bounded remote capability profile of that owner. The [owner plan](../crates/modules/rustok-messenger/docs/implementation-plan.md) sequences the work and carries the release gates.

The planned `rustok-messenger-gateway` binary lives under `crates/modules/` and deploys independently of `apps/server`. It owns WebSocket admission, validated session identity, heartbeat, bounded buffers, flow control, routing and shutdown. The selected Auth owner contract establishes identity and expiry. Messenger authorizes subscriptions and commands. Typed commands reach Messenger over authenticated gRPC with trusted context, deadlines and stable operation IDs. The owner returns a committed acknowledgement or a typed error. Client reconnect uses the owner sync API.

Use the canonical `outbox_iggy` profile for committed wakeups across the process boundary, with explicit delivery to each subscribed gateway node. The first topology has one gateway instance. The scale gate qualifies multiple nodes, their routing leases and fanout. Package the owner contracts and transport so the gateway's resolved build graph is free of the owner database implementation. Start with Axum and Tokio, the existing platform stack, and measure connection memory and throughput with explicit buffer and OS limits.

## Sources of truth and ownership

| State | Authority |
| --- | --- |
| Conversations, membership, message state and ordering, read markers, command receipts | Messenger's private PostgreSQL schema |
| Reconnect journal | Messenger's ordered, content-free change references and the current authorized owner state they resolve to |
| Event delivery, retry and replay | `rustok-outbox` and the platform event runtime. Iggy is a selectable relay when required |
| Connections, typing and presence | Separate Connection Gateway, bounded local sessions and, for multiple nodes, expiring distributed leases |
| Identity, media bytes and access descriptors, search indexes, notifications | Their existing platform owners. Messenger supplies its own audience and reference contracts |

## Invariants

- Preserve the existing `BIGINT` message identity and allocate new IDs in PostgreSQL. A separate positive, increasing per-conversation sequence orders committed changes. Public clients use a lossless string representation for IDs and cursor components wider than JavaScript's safe integer range.
- A committed send, edit, delete or membership change has an owner revision and a durable change reference. Current message rows carry the latest authorized state. The journal never retains an older body, and resolving an old reference cannot resurrect a deleted body.
- Read receipts represent how far a user has read in a conversation. The reconnect checkpoint represents what one client has applied. Advancing either does not advance the other.
- Every user command and protected read, including sync, resolves a trusted tenant and actor and applies current room policy. A new or rejoined private membership starts with access to new messages. A broader history grant would require an explicit owner policy. A system send is a separate trusted operation that clients cannot select.

## Non-goals

The first release covers direct messages, private groups, and their history. Large broadcast channels, cross-tenant conversations, calls, Matrix federation, and full E2EE are separate capabilities with their own readiness gates. Commerce and other business modules retain ownership of their workflows.

## Data, transaction, and concurrency boundary

Serialize room mutations on the same tenant-scoped conversation head. Reuse `rustok_outbox::idempotency` to admit the operation before opening the owner transaction. In one short PostgreSQL transaction, verify current authority and related records, allocate the next room sequence, apply the write, append the change reference, publish any required outbox event, and complete the receipt under the lease fence. Losing the lease aborts the write. A retry by the same authorized actor with the same key and normalized command returns the original acknowledgement identity. Reusing the key with a different command yields a conflict. An acknowledgement follows commit. A rejected or rolled-back command consumes no room sequence. A direct room has exactly two distinct tenant-admissible participants. A database constraint on the tenant and normalized participant pair serializes direct-room creation.

Use tenant and conversation constraints for membership, replies, and read markers. Read advances validate a visible message in the same room and use a conditional monotonic write. Membership revocation and room writes share the serialization boundary. This introduces contention within one hot room. Other rooms advance independently. Keep external calls outside this transaction and measure lock wait before changing the ordering design.

## Context dimensions

Propagate tenant, actor kind, current grants, trace and deadline from the trusted runtime. Room membership adds the domain authorization check. Scope subscriptions, receipts and cursors to tenant and room, and bind synchronization tokens to the membership generation. Apply the platform locale contract to channel metadata and UI strings. A messaging channel is a Messenger conversation type and has no implicit authority from a platform delivery channel.

## Events and projections

After commit, use the configured event runtime to deliver minimal references with stable event identity. Side-effect consumers such as search and notifications resolve current owner data and audience under their contracts, tolerate replays, and apply idempotent effects. The owner journal serves reconnect independently of transport offsets and event retention. One WS session per client multiplexes bounded, coalescible wakeups for the actor's conversation list and active rooms. The list wakes on new membership, removal, preview or unread changes, including when no room is open. Clients refresh the authorized list through bounded owner reads and fetch room content through the owner sync path. Presence and typing use short TTLs and are ephemeral.

## Failure semantics

Use bounded keyset pages over `(after, through]`. The owner fixes a committed upper watermark for the page chain and returns the last traversed revision. Entries outside the actor's visibility can be skipped while advancing only over the range actually traversed. Resolved upserts may reflect a newer current revision than the traversed entry. Clients merge by message revision and advance their checkpoint only through the server's traversed watermark, persisting applied state and the checkpoint together. An expired or incompatible checkpoint yields an explicit reset with a bounded authorized snapshot and its watermark from the same consistent view. Older visible history has its own pagination contract. A removed member receives no protected content on subsequent reads. Session expiry, revocation and client lag close or reset subscriptions, and reconnect is safe after a dropped hint, duplicate, or reordered delivery.

## Migration and cutover

Check installation and migration history before choosing the path. Preserve applied migrations and existing message identities. For existing conversations, backfill a deterministic baseline, initialize the room heads and advance the database ID allocator beyond all existing IDs under a write fence. Start a new sync epoch and force an authorized snapshot for incompatible cursors. Inventory and preserve existing audience restrictions and quarantine ambiguous or invalid legacy data. A baseline describes current state and cannot recover historical edits or deletes. Switch writers and readers as a coordinated cutover, with a rehearsed recovery boundary before enabling new writes.

## Alternatives considered

- An Iggy-first message store makes accepted writes, room membership changes, idempotency receipts and history depend on separate processing stages. It would require a new durability and authorization contract. The owner database already supplies the atomic boundary needed for the first release.
- Dedicated router, history and presence services would add independent deployment and recovery work before any measured bottleneck. A separate connection runtime can address socket scaling while leaving history and command authority together.
- Continuing to use Snowflake IDs as replay positions preserves a simpler schema but cannot express committed order or changes to existing messages.

## Verification

Use real PostgreSQL to exercise two writers, rollback, concurrent retries and revocation, monotonic receipts, migration from existing data, and paginated reconnect through edits and deletes. Verify contract parity across native and public transports and authorization through actual composition. Exercise separate owner and gateway processes with the real pinned Iggy profile. Restart the owner with sockets still connected, test gateway restart and catch-up, uncertain command outcomes, session expiry and slow-client bounds. Qualify the multi-gateway topology separately with a two-gateway failover and reconnect scenario.

## Consequences

PostgreSQL remains the write and sync dependency during an event or push outage. Socket lifetime and connection resource limits have an independent deployment boundary from the first release. This requires owner RPC, session authorization, real-broker delivery and cross-process recovery evidence before release. Hot-room contention, journal retention, long-lived connections and broker fanout become measurable constraints with distinct remedies. Support for a new storage or deployment profile requires separate migration, recovery and conformance evidence.
