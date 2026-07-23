# `rustok-notifications` module-local implementation gates

The canonical cross-module task order and status remain in
`crates/rustok-forum/docs/implementation-plan.md`. This file does not duplicate
that backlog; it records the owner-local gates that future notification slices
must preserve. The notifications program remains `in_progress` until maintainer-run verification and canonical-plan promotion are recorded. `NOTIFY-00`,
`NOTIFY-01`, `NOTIFY-03`, and `NOTIFY-07` are active partial tasks.

## Scope

Preserve the neutral producer boundary, owner-only persistence, optional-module
degraded behavior, mandatory recipient privacy policy, and module-owned UI
packages while the notifications product is implemented incrementally.

## Current State

The neutral API, bounded source/provider registries, deferred host factory
materialization, optional owner composition, and explicit admin/storefront
foundation states exist. Forum publishes live providers for
`forum.topic.created` and `forum.mention.user_added`.

The owner persistence migrations create notification, delivery-attempt, fan-out,
preference, digest, encrypted push-subscription, durable source-inbox, and
candidate-processing state for PostgreSQL and SQLite. Typed Rust/DB values,
tenant-composite recipient integrity, bounded payloads, stable
dedupe/idempotency, leases, retry timing, completion guards, and source-event
conflicts are enforced.

`NotificationFanoutService` durably accepts source events, materializes bounded
provider descriptors, and writes cursor pages of idempotent pending candidates.
`NotificationCandidateService` then requires preferences, an injected recipient
privacy policy, and recipient-specific source authorization before one final
in-app notification can be created. It creates no delivery attempt.

The server composes tenant-scoped Profiles privacy with concrete Social Graph
block/mute owner adapters inside `NotificationRecipientPolicyRuntime`. Relation
ports are ready in the baseline distribution, while candidate worker enablement
remains a separate explicit false gate. The production outbox runner, grouping,
channel delivery, retention/reconciliation, inbox APIs, and PostgreSQL runtime
evidence remain open.

## Milestones

### Boundary gate

- producer modules depend only on `rustok-notifications-api`;
- producer transactions never call the notifications owner synchronously;
- semantic descriptors contain bounded template data and target identity, not
  contact data or private source payloads;
- audience resolution is cursor-based and capped;
- candidate creation requires preference, recipient policy, and current source
  authorization;
- no allow-all recipient policy is supplied by the owner;
- target opening and delayed delivery recheck authorization/privacy;
- provider and module absence are explicit degraded states.

### Delivered in `NOTIFY-00B`

- optional owner/runtime/distribution composition exists but notifications remains
  outside tenant defaults;
- producer factories are materialized only after executable host services exist;
- factory/provider identity and build failures fail startup explicitly;
- Forum commands succeed without the notifications owner;
- Forum provides bounded source contracts for topic creation and direct user
  mentions;
- module-owned admin/storefront packages expose only foundation/unavailable state.

### Delivered in `NOTIFY-01A`

- module-owned PostgreSQL/SQLite migration
  `m20260721_000010_create_notification_persistence`;
- typed owner entities for notifications, delivery attempts, fan-out,
  preferences, digests, and push subscriptions;
- tenant-composite recipient/user integrity and actor/fan-out tenant guards;
- notification and command idempotency/dedupe indexes;
- bounded payload/cursor/error/secret fields and encrypted push material;
- SQLite and opt-in PostgreSQL invariant profiles.

### Delivered in `NOTIFY-01B/03A`

- migration `m20260722_000011_create_notification_source_inbox`;
- durable source event dedupe with changed identity conflict;
- recoverable source/job leases, retry state, bounded cursor fan-out, and one
  descriptor job per source event;
- idempotent pending candidates capped at 256 recipients per provider page;
- no final notification or delivery before policy;
- Forum direct-user-mention provider bound to exact immutable relation identity
  and current source visibility;
- machine contract and verifier under
  `contracts/notifications-source-fanout.json` and
  `scripts/verify/verify-notifications-source-fanout.mjs`.

### Delivered in `NOTIFY-03B/07A`

- migration `m20260722_000012_add_candidate_processing` adds candidate processing,
  retryable, processed, skipped, and failed states plus recoverable leases and
  retry timing for PostgreSQL and SQLite;
- SQLite rebuild preserves candidate rows, indexes, and tenant-integrity triggers;
- `NotificationCandidateService` claims one candidate and resolves preference
  precedence as exact source/type, exact source/wildcard type, wildcard source/
  exact type, then global wildcard;
- no matching preference preserves the current in-app default, while delivery
  mode `off` or disabled in-app delivery suppresses the candidate;
- `NotificationRecipientPolicy` is mandatory and has typed allow/suppress/error
  results for recipient, profile, block, mute, and tenant decisions;
- the owner provides no permissive policy implementation and reads no Profiles-
  owned private table;
- the source provider reauthorizes the target for the exact recipient before
  inbox creation;
- preferences are re-read in the final transaction to close a concurrent disable
  race;
- notification insert/replay equality and candidate completion share one
  transaction and an unexpired lease CAS;
- final notification dedupe remains tenant/recipient/source/event/type;
- no delivery attempt is created;
- SQLite source scenarios cover allow, exact-preference suppression, block
  suppression, unavailable target, retryable privacy failure, terminal replay,
  one notification, and zero deliveries;
- machine contract and verifier are
  `contracts/notifications-candidate-policy.json` and
  `scripts/verify/verify-notifications-candidate-policy.mjs`.

### Delivered in `NOTIFY-07B`

- Profiles exposes `ProfilePrivacyReadPort` and `ProfilePrivacyRuntime` as an
  owner-controlled tenant-scoped projection without exposing profile tables;
- inactive/missing profiles are unavailable, private/followers-only profiles are
  restricted for non-self actors, and owner read failures fail closed;
- Notifications publishes mandatory block and mute read-port runtime contracts
  without a permissive implementation;
- `ServerNotificationRecipientPolicy` evaluates profile, block, then mute state
  and maps owner decisions into typed candidate suppression;
- actor-bearing candidates receive retryable policy errors when either concrete
  relation provider is absent; missing providers never become implicit allow;
- `NotificationRecipientPolicyRuntime` records whether both relation ports are
  ready, and candidate-worker readiness remains false until they are;
- server host composition registers the policy runtime before source-provider
  materialization;
- machine contract and verifier are
  `contracts/notifications-recipient-policy-runtime.json` and
  `scripts/verify/verify-notifications-recipient-policy-runtime.mjs`.

### Delivered in `SOCIAL-01A / NOTIFY-07C`

- `rustok-social-graph` owns PostgreSQL/SQLite block and mute persistence with
  tenant-composite source/target user integrity;
- one stable relation identity stores current active state and monotonic revision;
- owner command ports require deadline, idempotency, source actor, and optional
  expected revision semantics;
- block privacy is strict when either direction is active, while mute remains
  directional from recipient to actor;
- server adapters implement Notifications block/mute ports through
  `SocialGraphPrivacyReadPort` without reading owner tables;
- the baseline distribution registers `SocialGraphModule` and relation policy
  readiness is true;
- candidate worker enablement remains explicitly false and is not inferred from
  provider readiness;
- machine contract and verifier are
  `crates/rustok-social-graph/contracts/social-graph-notification-policy.json`
  and `scripts/verify/verify-social-graph-notification-policy.mjs`.

### Remaining `NOTIFY-01` scope

- promote module-local migrations into verified global server migration
  composition;
- add retention, reconciliation, repair, and administrative command state;
- keep inbox, preference, digest, and delivery transports closed until matching
  owner commands are implemented.

### Remaining `NOTIFY-03` scope

- wire production outbox relay consumption into durable source enqueue;
- explicitly enable, compose, and start the production candidate worker;
- add grouping policy and bounded moderator-audience expansion;
- enqueue channel work only after policy acceptance and outside owner provider
  calls;
- add PostgreSQL lease/concurrency/retry evidence and DLQ/replay controls.

### Remaining `NOTIFY-07` scope

- add tenant-specific notification restrictions beyond tenant identity guards;
- add block/mute management transports and social relation change events;
- recheck privacy and source authorization on inbox open and delayed delivery;
- redact/archive notifications after source/profile state changes and reconcile
  unread counts;
- add executable blocked/private/deleted and cross-tenant evidence.

### UI gate

Admin and storefront packages remain module-owned. Until inbox APIs exist, they
expose only bootstrap/unavailable states and must not invent unread counts or
store shadow inbox state in the host.

## Verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --all-targets
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_sqlite -- --nocapture
cargo test -p rustok-social-graph --test privacy_sqlite -- --nocapture
cargo test -p rustok-forum --test notification_source_sqlite -- --nocapture
NOTIFICATIONS_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test -p rustok-notifications --test persistence_postgres -- --nocapture --test-threads=1
node scripts/verify/verify-notifications-foundation.mjs
node scripts/verify/verify-notifications-foundation.test.mjs
node scripts/verify/verify-notifications-runtime.mjs
node scripts/verify/verify-notifications-runtime.test.mjs
node scripts/verify/verify-notifications-persistence.mjs
node scripts/verify/verify-notifications-persistence.test.mjs
node scripts/verify/verify-notifications-source-fanout.mjs
node scripts/verify/verify-notifications-candidate-policy.mjs
node scripts/verify/verify-notifications-recipient-policy-runtime.mjs
node scripts/verify/verify-social-graph-notification-policy.mjs
cargo xtask module validate notifications
```

The commands above are the maintainer verification set. They were not executed
while publishing the `SOCIAL-01A / NOTIFY-07C` source slice.

## Update Rules

- keep task status in the canonical forum/notifications program plan;
- update this file only for owner-local gates or verified runtime shape;
- never move producer subscriptions, contact data, or channel SDKs into this
  owner;
- never add synchronous notification calls to producer transactions;
- never create final notification rows before preference/privacy/source policy;
- never create channel delivery work in candidate finalization;
- add persistence and UI behavior only with matching owner contracts, migrations,
  degraded-mode notes, and verification commands.
