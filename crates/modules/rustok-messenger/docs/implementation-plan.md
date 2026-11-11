---
id: doc://crates/modules/rustok-messenger/docs/implementation-plan.md
kind: development_plan
language: en
status: active
---

# Messenger implementation plan

## Scope and decision gate

Deliver a reliable private messenger first. A user can open a direct or private group conversation, send and retry a text message, reconnect without losing committed changes, and read the same history on a second device. Edits, deletions, membership changes and read markers have defined behavior. Admins can inspect tenant-scoped operational health and perform authorized channel or membership actions without browsing private conversations by default.

The [proposed Messenger ADR](../../../../DECISIONS/2026-11-11-messenger-write-and-sync.md) owns the target write, authorization and sync contract. Accept it before starting the cutover. This plan applies the [embedded-first FBA sequence](../../../../docs/research/fluid-backend-architecture-unified-plan.md) and the [platform event contract](../../../../docs/architecture/event-flow-contract.md). The [Workplace proposal](../../../../docs/architecture/workplace-collaboration-superapp-plan.md) carries broader product ideas. Calls, collaborative documents, calendars and commerce workflows stay with their respective owners.

## Source baseline

The Messenger source baseline was reviewed at `e16f5a4ae92b92f85361e42d31eb0522a49f0bf7`. The deployment and package boundaries were reconciled against local `main` at `2a58754812a9d968930ebc1ac19677c2f8917f50` on 2026-11-11. The source review establishes an embedded boundary and a partial scaffold. Runtime, privacy, frontend parity and scale remain open.

| Surface | Present in source | Production blocker |
| --- | --- | --- |
| Owner and persistence | `MessengerModule`, `MessengerService`, `MessengerPort`, five SeaORM tables and a transactional send outbox write | The service assigns a Snowflake ID before commit and FastSync treats it as an ordered checkpoint. Edits and deletes change the existing row. Concurrent cursor writes can race and deleted bodies are still returned in history and sync. |
| Authorization | REST, GraphQL and native paths have permission checks and the service has membership checks | Public REST accepts a `System` message kind that skips membership in the service. Cursor writes lack tenant-scoped room admission. Some related-row constraints and membership transitions require transaction-level enforcement. |
| Realtime | Axum upgrade and process-local `MessengerRoomHub` | REST and GraphQL issue adapter-local broadcasts and can construct separate hubs. Native and port writes bypass those broadcasts. A WS edit can broadcast the body to the socket's room even if the edited message belongs to another room. The hub skips lagged frames and lacks a reconnect/revocation contract. |
| Events and search | `MessengerMessageDispatched`, a Notifications provider and a local message search query | Notifications checks deletion when loading the source. Validate privacy across materialization, delayed delivery and target opening. The optional Iggy dispatcher prepares a payload without publishing it. Search has no proven owner event projection or uniform private audience gate. |
| UI | Manifest declares Leptos admin `messenger` and storefront `messages`, plus a floating widget. Both packages have native and GraphQL adapter source | Prove generated composition, schema agreement, send/retry and sync in the live entry paths. Next.js packages and equivalent routes are absent. Replace placeholder E2EE and federation status with truthful capability state. |
| Advanced profiles | In-memory device/prekey registry, envelope types and Matrix translation code | Durable tenant-bound keys, client crypto, authenticated federation and real broker recovery require separate implementation and evidence. |

The detailed owner surfaces are in the [crate API](../CRATE_API.md), [module manifest](../rustok-module.toml), [service](../src/services/messenger_service.rs), [realtime adapter](../src/controllers/realtime.rs) and [streaming adapter](../src/streaming.rs). Check actual installation and schema state before treating the scaffold as unused. The module is listed in the repository's default tenant composition.

## Module decomposition

Keep the write, history and membership responsibilities within `rustok-messenger` in the [native FBA layout](../../../../docs/backend/module-backend-implementation.md). Give conversation and membership, timeline commands, and synchronization and read state separate application modules during the owner refactor. Share owner transaction logic where invariants intersect. Implement the connection adapter in the separate `rustok-messenger-gateway` crate from MSG-03. It consumes a narrow, database-free owner contract. Use an owner contracts package if the gateway cannot import that contract through an independent build of the owner package. Select process placement using the release and scale gates below.

| Boundary | Owns | Interfaces and dependencies |
| --- | --- | --- |
| Conversations and membership | Conversation identity and lifecycle, canonical DM pair, roles, membership generation, history policy, localized channel metadata | Uses trusted `rustok-api` context and existing auth/user resolution. Other modules can request a contextual room through an authorized `MessengerPort` operation. The linked entity remains with its owner. |
| Message timeline | Send, edit, delete, reply, current message state, immutable change references and operation receipts | Uses the owner PostgreSQL/SeaORM transaction and `rustok-outbox`. All writers go through this boundary, including trusted system sends and future bridge adapters. |
| Sync and read state | Timeline pagination, reconnect checkpoints and snapshots, unread calculation and read receipts | Reads owner state under current membership. A reconnect checkpoint belongs to one client and its local cache. Read state belongs to the user in a room. |
| Realtime | Socket lifecycle, bounded queues and wakeups, local fanout | One Axum/Tokio implementation in `rustok-messenger-gateway`. The baseline host supplies the local owner and outbox adapters. Standalone deployment and multi-node routing qualify separately. |
| Integrations | Owner audience and reference adapters for Notifications, Media, Search and linked entities | Reuse `rustok-notifications`, `rustok-media`, `rustok-search` and entity-owner ports. Domain entities and private storage remain within their owners. |
| Clients | Web workspace and widget, admin workspace, later mobile offline state | Leptos `admin/` and `storefront/` keep `core`, `transport` and render adapters. Next.js keeps module packages and thin host composition. Share the API schema and behavior fixtures between ecosystems. |

### Release profiles

The baseline links the gateway library into `apps/server` and calls the owner in process. Agree on the first release workload and headroom before choosing its deployment profile. Measure concurrent sockets, reconnect bursts, memory and queue occupancy, message rates and room sizes, write latency and commit-to-visible latency in the embedded profile. Compare those results with the release budget.

Select standalone deployment when sockets must survive owner deploys or measured contention threatens the release budget. A thin binary in the same crate composes the gateway library with the remote owner and wakeup adapters. If independent deployment is a first-release requirement, qualify the single-gateway part of MSG-06 and use it for the first production rollout. A second gateway triggers inter-node routing and lease tests. Both profiles use the same WebSocket implementation, owner rules and client wire protocol.

### Gateway composition and activation

| Scope | Required behavior |
| --- | --- |
| Build | Select the optional gateway dependency for a server artifact with `mod-messenger` and the embedded WebSocket placement. A standalone artifact owns the library in the remote profile. Compare the generated composition and resolved dependency graphs for embedded, standalone and Messenger-free artifacts. |
| Installation | Bind one public WebSocket placement to the Messenger owner. Compose the library with the local owner in the embedded profile. For standalone, admit and start the gateway through the selected distribution topology with its compatible remote owner binding. Resolve the existing role and surface contract as part of standalone qualification. |
| Tenant | Use the same `messenger` enablement authority for HTTP, GraphQL, native and socket operations. Check enablement and room access before registering a socket or subscription. Close the affected sessions and release leases on disable. Revalidate within bounded session lifetimes and fail closed for protected operations when current authority cannot be established. Re-enable through the Messenger lifecycle and admit new sessions. |

Gateway depends on the Messenger capability contract. The host binds that contract to the owner, with a local adapter in process or an authenticated remote adapter. Keep session and routing resources scoped to admitted sessions. Build inclusion controls binary contents and tenant enablement controls which tenants can use the composed capability.

## Milestones

### MSG-01 — Establish the safe owner boundary

- **Work.** Inventory live installations and applied migrations, public and internal writers, generated HTTP/GraphQL composition, tenant enablement, and both web surfaces. Trace REST, GraphQL, native server functions, WS, `MessengerPort`, Notifications and the dormant Matrix entry point. Record supported conversation types and roles, join/rejoin history, last-owner and tenant policy, reply visibility, redaction, and user/system command separation. Apply the immediate authorization and redaction fixes before exposing the existing endpoints to real users. Use the existing module enablement and release controls while readiness gates are open.
- **Dependencies and configuration.** Use current PostgreSQL and the platform auth, policy and tenant contexts. Check `modules.toml`, `rustok-module.toml` and `messenger-fba-registry.json`. Reject encrypted-mode and federation activation until the corresponding profiles pass their gates. Inspect any existing encrypted or ambiguous records during migration preflight and handle them fail closed.
- **Gate.** PostgreSQL and transport probes reject cross-tenant operations, cross-room socket actions, public system sends, replies to inaccessible messages, reads after revocation and deleted content leakage, including notification previews. Concurrent owner changes cannot leave a room ownerless. Verify a disabled module or capability through its actual entry paths.
- **Risk.** A generated route or dependency proves composition intent. Demonstrate the actual resolved tenant, actor and module gate on the running profile before a rollout.

### MSG-02 — Make writes, history and reconnect durable

- **Work.** Implement the ADR's transaction and ordered journal under `rustok-messenger`. Allocate existing `BIGINT` IDs in PostgreSQL and use a lossless wire representation. Give sync its own per-room sequence. Add tenant-scoped conversation heads, revisions and change references, direct-pair uniqueness, canonical operation receipts with normalized fingerprints, and constraints for member/message/reply/read references. Move all supported writes through the same owner entry point. A direct send uses the canonical pair and the same transaction for first-message creation. Record explicit read-through state, distinct from reconnect progress. Define unread counts over visible, nondeleted incoming messages and derive them from the owner initially.
- **Dependencies and configuration.** Use SeaORM with PostgreSQL and `rustok-outbox`. Set and validate history, sync page and retention bounds. Document retry receipt retention as the supported client retry window. Implement the migration/preflight and the existing-data snapshot path from the ADR if needed. Coordinate all reader and writer changes in one cutover with a tested recovery boundary.
- **Gate.** Real PostgreSQL tests cover two concurrent senders, higher/lower ID commits, duplicate retry after a lost response, key reuse with a different payload, rollback without an outbox entry or an advanced head, last-owner and membership races, monotonic read receipts, and tenant/room constraints. Reconnect tests traverse multiple pages through edits, deletes and membership changes, apply tombstones without revealing old bodies, and reset cleanly when a cursor is outside retention. Test the migrated baseline if the scaffold already has data.
- **Risk.** A journal page advances by the server's traversed revision. Advancing to the maximum message ID, the newest returned body revision or a broker offset can skip committed changes. Keep external calls out of the room write transaction and record the hot-room lock cost.

### MSG-03 — Connect public transports and embedded realtime

- **Work.** Map HTTP, GraphQL, native server functions and WebSocket to the same owner commands, errors, cursor and pagination semantics. Bring the existing socket entry point onto the gateway crate's Axum/Tokio implementation and compose it in `apps/server` with the local owner adapter and outbox listener. Retire resolver/controller broadcasts and the inert publisher. Multiplex authorized, coalescible wakeups for conversation lists and active rooms on one WebSocket per client. Subscribe before catch-up and perform bounded foreground reconciliation to recover a lost final hint. Expire sessions, close affected subscriptions on revocation or lag, and release connection and room resources.
- **Dependencies and configuration.** Reuse the existing Auth owner, Axum, Tokio and the default `outbox` profile. Use platform authentication for the upgrade and browser origin checks where applicable. Configure buffer, frame, heartbeat, rate and graceful shutdown bounds, tenant/actor connection caps and jittered reconnect backoff. Keep deployment and transport code outside owner transaction logic.
- **Gate.** Verify the gateway library builds independently of Messenger persistence and that a composition without Messenger omits its gateway dependency and route. In the embedded composition, REST, GraphQL, native, trusted port and socket writes produce the same persisted effects and wake a second client. A recipient discovers a new DM while viewing another room or after reconnect. Verify that a disabled tenant cannot upgrade, that disabling an active tenant closes its sessions without disturbing an enabled tenant, and that re-enable admits new sessions. Test owner restart with client reconnect, lost or reordered hints, uncertain write outcomes, slow clients, room isolation, revocation and session expiry. Establish a measured capacity baseline with bounded memory and queues and a reconnect burst.
- **Risk.** Owner restart closes embedded sockets. Reconnect recovers committed changes from PostgreSQL. A send acknowledgement means the owner committed. Socket admission and packet forwarding have separate meanings if the protocol exposes them.

### MSG-04 — Release the same private messaging workflow on both web stacks

- **Work.** Complete the Leptos storefront workspace and contextual widget using one client sync model. Implement equivalent Next.js storefront and admin module packages with manifest-driven host routes and navigation. Preserve the `messages` storefront and `messenger` admin route identities and host locale/dashboard prefixes. Share fixtures for send/retry, lossless identifiers, unread/read, membership changes, redaction, stale checkpoints and disabled/error states. Admin UI exposes health and authorized operations from real data. Keep optimistic state and retry identity in the UI core, native `#[server]` plus GraphQL behind the Leptos transport facade, and schema-checked GraphQL in Next.js. Reuse the platform UI and i18n stack.
- **Dependencies and configuration.** Follow the [FFA contract](../../../../docs/UI/module-package-architecture.md) and [GraphQL architecture](../../../../docs/UI/graphql-architecture.md). Use generated schema/composition surfaces and platform Fluent/locale policies. Provision test tenants and users with distinct roles. No new service is required.
- **Gate.** Each advertised route resolves on both stacks. Two users can create a DM/group, send with lost-ack retry, read, edit, delete and recover after reconnect. Show the same permissions, failure states and unread progression. Verify GraphQL documents against the generated schema and run both supported Leptos transport modes. Complete the release with privacy regression evidence from MSG-01 through MSG-03.
- **Risk.** Rendering a shell or passing a pure view-model test does not establish a working UI. Record entry-path evidence for each stack and role. MSG-04 is the first private web release gate.

### MSG-05 — Add practical collaboration capabilities as independent slices

| Slice | Work and dependencies | Gate |
| --- | --- | --- |
| Channels and threads | Add channel lifecycle, localized metadata and bounded thread/reply queries in Messenger. Contextual rooms use verified entity-owner access and membership, with no reads of foreign private tables. | Enforce visibility, membership and parent scope in each path. Verify locale fallback and equivalent web actions. |
| Notifications and mobile | Harden the existing `rustok-notifications` source. Materialize current eligible recipients, respect mute preferences, deduplicate side effects and use safe deep links. Add the Flutter client on the same contracts with per-account cache, transactional checkpoints and queued operation IDs. | Replay and audience changes do not expose previews to departed members. The mobile client survives kill/restart and offline send. Web/mobile push uses the existing Notifications provider capabilities and owner-resolved targets. |
| Attachments | Reuse the Media owner upload/finalize and delivery contracts. Messenger stores an authorized asset reference and checks tenant, scan/readiness and room audience before exposing a delivery descriptor. Use Media-owned storage and job configuration. | Cross-room, expired, quarantined and deleted assets cannot be downloaded through Messenger. Test cleanup and retained references. |
| Search | Add a versioned owner projection into `rustok-search` from committed references with tombstone handling and reconciliation. Keep private-room audience enforcement in the owner contract on query/open, including counts and snippets. Configure the existing search backend. | Delayed or replayed index events cannot restore deleted content or leak it after revocation. Encrypted conversations stay outside server content indexes. |
| Typing and presence | Start as bounded in-process ephemeral state with authorization, TTL and coalescing. Use a shared lease backend only in the distributed profile. | Expiry, leave and restart clear stale state. Backpressure cannot block the durable write path. |

Deliver these slices one at a time and verify both web clients with each exposed capability. Push, attachments and search have distinct retry and privacy boundaries. They can follow the first web release without changing message durability.

### MSG-06 — Qualify standalone and multi-node realtime when needed

#### Standalone gateway

- **Work.** Apply the release-profile trigger. Add the standalone entry point to the existing gateway crate and reuse its socket, session and fanout code. Supply readiness, graceful shutdown, an authenticated gRPC adapter for the owner port and the platform `outbox_iggy` wakeup source. Preserve tenant/actor context, deadlines and stable operation IDs across RPC. Verify the standalone dependency graph excludes the database implementation and route public WebSocket traffic to the selected deployment.
- **Gate.** Prove admitted topology wiring and that the Messenger server artifact excludes the gateway library in this placement. Refuse new sessions when the owner binding is absent, incompatible or unavailable. Run the tenant enable, disable and re-enable cases through the remote profile. Qualify the pinned broker and global event profile with affected consumers, including their relay lag, failure and recovery behavior. Verify owner restarts preserve gateway sockets while commands return retryable outcomes during the outage. Verify reconnect, broker outage, lost acknowledgements and gateway restart with real owner, gateway and broker processes. Compare the standalone profile with the embedded baseline under the same workload. Document profile rollout and recovery before enabling it.

#### Routing across gateways

- **Work.** Add a router worker when clients can be connected to multiple gateway nodes. Messenger determines the authorized audience. The worker maps each tenant-scoped recipient to every node with a live session. Keep bounded leases keyed by tenant, actor and node with heartbeat, expiry and generation-checked removal. Select a shared lease backend for that topology. Measure fanout and relay lag before assigning the router its own process.
- **Gate.** Verify two gateways, same-user devices on different nodes, lease expiry and node loss, rebalance, duplicates, slow receivers and rolling deploys. A single Iggy consumer group distributes work, so test the explicit per-node fanout. Pin the routing, offset, retention, retry and DLQ settings, and prove partition affinity against the selected adapter if routing relies on it. Durable consumers commit offsets after their handling outcome and apply effects idempotently by event identity.

Database lock wait, history read latency and storage retention are separate from socket or fanout capacity. Use those measurements to decide whether PostgreSQL partitioning is warranted. A different history store requires its own ADR and atomicity, migration and recovery proof.

### MSG-07 — Treat E2EE and federation as separate release profiles

- **Work.** For E2EE, require a reviewed threat model and interoperable client protocol, tenant-scoped durable public devices/prekeys, atomic one-time key claims, verification and revocation, rotation on membership/device change, and explicit device recovery behavior. Encrypt attachments client-side and keep private keys and plaintext at the client. A supported encrypted conversation type is immutable unless a separately designed migration defines a safe transition. For federation, decide identity and tenant mapping, inbound authorization, external IDs and idempotent replay, and verify signed events through the canonical writer. Decide whether to support Matrix based on a separate owner contract.
- **Dependencies and configuration.** Select supported crypto and bridge dependencies through workspace review after the protocol decision. Verify restart and multi-device key behavior against the actual clients. Use existing secret, notification and media facilities under the selected privacy policy.
- **Gate.** Known vectors and interoperability tests, a revoked-device/member scenario, replay, restart, attachment handling and end-to-end checks for plaintext exclusion from the server, logs, events, previews and indexes. Any federated profile needs cross-server interoperability and a tenant-isolation review before enablement.
- **Risk.** The existing key registry and envelope types are building blocks. They do not establish an encrypted client protocol or authenticated Matrix federation.

## Verification and execution order

One developer can take MSG-01 and MSG-02 as the first two delivery increments, then complete MSG-03 and MSG-04 before starting optional slices. Parallelize independent review and fixture design while the owner transaction contract is being finalized. Keep one owner for writer/schema changes and merge at each gate with refreshed `main` and recorded evidence.

- **Owner checks.** `cargo check -p rustok-messenger --features graphql` and `cargo test -p rustok-messenger --lib --features graphql`. Add named PostgreSQL integration targets for migration, concurrent operations, outbox, sync and security cases. Exercise the real owner adapters and persisted state in these targets.
- **Composition and contracts.** Run `cargo xtask module validate messenger`, `npm run verify:manifest`, and the architecture dependency guard when crossing crate/layer boundaries. Follow the [backend verification guide](../../../../docs/backend/module-backend-verification.md) and run the targeted composed-profile probe. Regenerate schema and registry evidence for changed public surfaces.
- **Client checks.** Run scoped Rust admin/storefront unit and feature checks, then the affected Next package type checks and the route/behavior probes. Use the [FFA verification guide](../../../../docs/UI/module-package-verification.md). Run `npm run verify:ui:parity` if shared UI primitives or tokens change.
- **Release evidence.** Record exact commands, results, profile and remaining CI/maintainer checks in the owner documentation. Full workspace suites, sustained load, long E2E and rollout drills are maintainer/CI gates. The source baseline above carries no new runtime test results.
