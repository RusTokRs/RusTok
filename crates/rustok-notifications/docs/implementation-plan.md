# `rustok-notifications` module-local implementation gates

The canonical cross-module roadmap remains
`crates/rustok-forum/docs/implementation-plan.md`. This ledger records the
owner-local boundaries that every Notifications slice must preserve. The program
remains `in_progress` until maintainer-run verification and canonical promotion
are recorded.

## Scope

Preserve the neutral producer boundary, owner-only persistence, optional-module
degraded behavior, mandatory recipient privacy, tenant capability enforcement,
and module-owned UI packages while inbox and delivery products are implemented
incrementally.

## Current state

Forum publishes live neutral providers for `forum.topic.created` and
`forum.mention.user_added`. Notifications owns five ordered PostgreSQL/SQLite
migrations covering persistence, durable source intake, candidate processing,
outbox acceptance receipts, and permanent intake quarantine.

The runtime pipeline has three independent, default-off stages:

1. outbox envelope intake into `notification_source_inbox`;
2. source descriptor materialization and bounded audience fanout;
3. recipient preference/privacy/source-policy candidate processing.

The server starts these stages in intake → fanout → candidate order. Fanout and
candidate workers expose tenant-scoped work and recheck effective policy before
foreign provider calls. Disabled work receives 300-second durable backoff;
temporary policy lookup failure receives 30 seconds.

Candidate pre-claim resolution captures one effective-policy snapshot containing
the deterministic revision and manifest default-enabled module set. The final
notification transaction invokes an injected commit guard that locks the
Modules-owned `module.lifecycle` cursor and resolves tenant overrides with that
observed manifest input on the same transaction. No manifest/pool read occurs
while the final transaction is active. PostgreSQL lifecycle tenant toggles advance
the cursor inside their tenant-state transaction, serializing candidate commit and
tenant enable/disable by commit order.

## Invariants

- producer modules depend only on `rustok-notifications-api`;
- producer transactions never call the Notifications owner synchronously;
- Notifications never reads producer-private or Modules-private tables;
- executable hosts decode envelopes and compose cross-owner policy ports;
- audience resolution is cursor-based and capped at 256 recipients per page;
- final notification creation requires preference, privacy, current source
  authorization, current tenant enablement, and matching policy revision;
- no allow-all recipient policy exists;
- disabled or unresolved tenant capability fails closed before provider calls;
- tenant-policy deferral leaves later work reachable in bounded selection;
- server workers never read Notifications private tables directly;
- final candidate transactions do not open a second connection for manifest reads;
- PostgreSQL lifecycle tenant toggle and final candidate commit share one cursor
  serialization point;
- delivery work remains outside candidate finalization;
- worker enablement is never inferred from provider readiness.

## Delivered milestones

### `NOTIFY-00B`

- optional owner/runtime/distribution composition;
- deferred provider factory materialization through `HostRuntimeContext`;
- duplicate or mismatched provider identity fails startup;
- Forum commands remain independent from Notifications availability;
- admin/storefront packages expose explicit foundation/unavailable states.

### `NOTIFY-01A`

- migration `m20260721_000010_create_notification_persistence`;
- typed notification, delivery, fanout, preference, digest, and encrypted push
  entities;
- tenant-composite recipient integrity, dedupe, bounded payloads, leases, and
  encrypted endpoint storage;
- SQLite and opt-in PostgreSQL invariant evidence.

### `NOTIFY-01B / NOTIFY-03A`

- migration `m20260722_000011_create_notification_source_inbox`;
- durable source identity and changed-replay conflict detection;
- recoverable source/job leases and bounded cursor fanout;
- one descriptor job per source event and idempotent pending candidates;
- no final notification or delivery before policy;
- contract `contracts/notifications-source-fanout.json` and verifier
  `scripts/verify/verify-notifications-source-fanout.mjs`.

### `NOTIFY-03B / NOTIFY-07A`

- migration `m20260722_000012_add_candidate_processing`;
- recoverable candidate leases, retry timing, and terminal states;
- exact source/type preference precedence before wildcards;
- mandatory injected recipient policy with typed suppression/error outcomes;
- recipient-specific source authorization and final-transaction preference recheck;
- idempotent notification insert plus candidate completion in one lease-CAS
  transaction;
- zero delivery attempts;
- contract `contracts/notifications-candidate-policy.json` and verifier
  `scripts/verify/verify-notifications-candidate-policy.mjs`.

### `NOTIFY-07B`

- Profiles owner privacy read port and runtime;
- mandatory Notifications block/mute runtime contracts;
- server policy order profile → block → mute;
- missing relation providers fail closed;
- contract `contracts/notifications-recipient-policy-runtime.json` and verifier
  `scripts/verify/verify-notifications-recipient-policy-runtime.mjs`.

### `SOCIAL-01A / NOTIFY-07C`

- Social Graph PostgreSQL/SQLite block and mute persistence;
- tenant-composite relation integrity and monotonic revisions;
- owner command/read ports;
- server adapters into Notifications policy contracts;
- relation-policy readiness true while candidate enablement remains separate;
- contract `crates/rustok-social-graph/contracts/social-graph-notification-policy.json`.

### `NOTIFY-03C`

- bounded `NotificationCandidateWorker`, default batch 32 and hard maximum 64;
- stable pending/due-retry/expired-processing selection;
- canonical service owns every claim and completion CAS;
- default-off host flag `RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED`;
- shared shutdown checks between candidates;
- contract `contracts/notifications-candidate-worker.json` and verifier.

### `NOTIFY-03D`

- migrations `m20260723_000013_add_outbox_intake_receipts` and
  `m20260723_000014_add_outbox_intake_rejections`;
- owner intake selects supported committed `sys_events` envelopes without relay
  status coupling;
- event decoding is injected by the executable host;
- semantic source identities for topic-created and mention events;
- source inbox and accepted receipt commit in one transaction;
- permanent invalid envelopes enter owner-local quarantine;
- accepted replay re-decodes and validates full semantic identity;
- accepted/rejected outcomes are mutually exclusive;
- default-off host flag `RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED`.

### `NOTIFY-03E`

- bounded `NotificationFanoutWorker`, default/hard batch 32/64 and page 256;
- tenant-scoped source/job work projections;
- stable selection without acquiring leases;
- canonical `NotificationFanoutService` owns every claim/page transition;
- default-off host flag `RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED`;
- effective policy checked before descriptor/audience provider calls;
- SQLite evidence covers bounded fanout and zero final delivery rows.

### `NOTIFY-03F`

- `NotificationFanoutPolicyDeferral` defines tenant-disabled and
  policy-unavailable outcomes;
- disabled work enters `retryable_error` for 300 seconds; lookup failures receive
  30 seconds;
- CAS increments attempts, persists stable metadata, clears leases, and prevents
  bounded queue starvation;
- SQLite evidence is `tests/fanout_policy_deferral_sqlite.rs`.

### `NOTIFY-03G`

- `NotificationCandidateWorkItem` exposes candidate and tenant IDs while keeping
  persistence private;
- the server resolves effective tenant policy before every candidate claim;
- disabled work invokes neither recipient policy nor source provider;
- candidate CAS backoff mirrors fanout 300/30-second semantics;
- SQLite evidence proves tenant-scoped selection, queue-head advancement, retry
  metadata, and zero notification rows.

### `NOTIFY-03H`

- public `NotificationTenantCapabilityCommitGuard` request/decision/error contract;
- guarded `NotificationCandidateService` and `NotificationCandidateWorker`
  constructors preserve trusted compatibility paths while production uses guarded
  paths only;
- pre-claim `EffectiveModulePolicyService::resolve_snapshot` forwards exact policy
  revision and manifest default-enabled module set;
- final transaction validates lease before commit guard and runs guard before
  preference recheck or notification insert;
- the commit request validates and carries observed manifest defaults, so the final
  transaction never opens a second pool connection to reload the manifest;
- Modules owner exposes `lock_and_resolve_static_policy_in_transaction` and keeps
  all `tenant_modules` reads outside server/Notifications;
- PostgreSQL guard locks `module.lifecycle` cursor with `FOR UPDATE`;
- production lifecycle state transition advances the same cursor in its transaction;
- disabled/revision-changed/guard-error outcomes roll back notification insert and
  enter durable candidate retry;
- SQLite evidence covers transaction-bound policy resolution and revision rejection
  rollback; PostgreSQL contention evidence remains maintainer-owned;
- candidate worker contract schema 6 and candidate policy contract schema 8 record
  the narrow lifecycle serialization and connection-safety guarantees.

## Remaining `NOTIFY-01`

- promote module-local migrations into verified global server migration
  composition;
- retention, reconciliation, repair, quarantine replay/purge, and administrative
  command state;
- keep inbox, preference, digest, and delivery transports closed until matching
  owner commands exist.

## Remaining `NOTIFY-03`

- serialize active-manifest, artifact-security, maintenance, and node-readiness
  policy mutations with final candidate commits;
- grouping policy and bounded moderator-directory expansion;
- channel work enqueue only after candidate policy acceptance;
- PostgreSQL cursor/lease/contention/retry evidence;
- worker health, queue lag, retry, and quarantine metrics before default deployment
  enablement.

## Remaining `NOTIFY-07`

- tenant restrictions beyond effective module capability;
- block/mute management transports and relation change events;
- privacy and source rechecks on inbox open and delayed delivery;
- redaction/archive reconciliation after source/profile changes;
- executable blocked/private/deleted and cross-tenant evidence.

## UI gate

Admin and storefront remain module-owned. Until inbox APIs exist, they expose only
foundation/unavailable states and must not invent unread counts or shadow inbox
storage.

## Maintainer verification set

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-modules --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --all-targets
cargo test -p rustok-modules --test policy_commit_guard_sqlite -- --nocapture
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_worker_sqlite -- --nocapture
cargo test -p rustok-notifications --test outbox_intake_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_worker_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_policy_deferral_sqlite -- --nocapture
cargo test -p rustok-social-graph --test privacy_sqlite -- --nocapture
cargo test -p rustok-forum --test notification_source_sqlite -- --nocapture
NOTIFICATIONS_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test -p rustok-notifications --test persistence_postgres -- --nocapture --test-threads=1
node scripts/verify/verify-notifications-foundation.mjs
node scripts/verify/verify-notifications-runtime.mjs
node scripts/verify/verify-notifications-persistence.mjs
node scripts/verify/verify-notifications-source-fanout.mjs
node scripts/verify/verify-notifications-candidate-policy.mjs
node scripts/verify/verify-notifications-recipient-policy-runtime.mjs
node scripts/verify/verify-social-graph-notification-policy.mjs
node scripts/verify/verify-notifications-candidate-worker.mjs
node scripts/verify/verify-notifications-outbox-intake.mjs
node scripts/verify/verify-notifications-fanout-worker.mjs
cargo xtask module validate notifications
```

These commands were not executed while publishing the
`NOTIFY-03D/03E/03F/03G/03H` source slices. `Cargo.lock` was not regenerated because
this work does not change the package dependency graph.

## Update rules

- keep canonical program status in the cross-module plan;
- never move producer subscriptions, contact data, or channel SDKs into this owner;
- never add synchronous notification calls to producer transactions;
- never create final notification rows before tenant/preference/privacy/source
  policy;
- never create channel delivery work in candidate finalization;
- add persistence or UI behavior only with matching contracts, migrations,
  degraded-mode notes, and verification commands.
