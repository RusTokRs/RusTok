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

The runtime pipeline now has three independent, default-off stages:

1. outbox envelope intake into `notification_source_inbox`;
2. source descriptor materialization and bounded audience fanout;
3. recipient preference/privacy/source-policy candidate processing.

The server starts these stages in intake → fanout → candidate order. Each stage
uses the shared stop signal and its own explicit environment flag. Fanout checks
the authoritative effective module policy for the exact tenant before every
source or job claim. No fanout stage creates final notifications or delivery
attempts; candidate finalization creates at most one in-app row and no channel
work.

## Invariants

- producer modules depend only on `rustok-notifications-api`;
- producer transactions never call the Notifications owner synchronously;
- Notifications never reads producer-private tables;
- executable hosts decode platform envelopes and compose cross-owner policy;
- audience resolution is cursor-based and capped at 256 recipients per page;
- final notification creation requires preference, privacy, and current source
  authorization;
- no allow-all recipient policy exists;
- disabled or unresolved tenant capability fails closed before provider calls;
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
- recipient-specific source authorization and final-transaction preference
  recheck;
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
- stable `created_at/id` selection of pending, due retryable, and expired
  processing candidates;
- canonical service owns every claim and completion CAS;
- default-off host flag `RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED`;
- shared shutdown checks between candidates;
- contract `contracts/notifications-candidate-worker.json` and verifier
  `scripts/verify/verify-notifications-candidate-worker.mjs`.

### `NOTIFY-03D`

- migration `m20260723_000013_add_outbox_intake_receipts`;
- migration `m20260723_000014_add_outbox_intake_rejections`;
- owner intake selects committed supported `sys_events` envelopes without reading
  or mutating relay status;
- event decoding is injected by the executable host, so the owner has no direct
  `rustok-events`, `rustok-outbox`, or producer dependency;
- root topic identity is `topic_id/1`; sealed mention identity is envelope ID plus
  relation revision;
- source inbox and accepted receipt commit in one transaction;
- permanent invalid envelopes enter durable owner-local quarantine, retryable
  failures receive no terminal record, and both outcomes are anti-joined from
  later selection;
- accepted and rejected terminal outcomes are mutually exclusive; PostgreSQL uses
  a per-event transaction advisory lock and both backends enforce cross-table
  insert guards;
- default-off host flag `RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED`;
- contract `contracts/notifications-outbox-intake.json` and verifier
  `scripts/verify/verify-notifications-outbox-intake.mjs`.

### `NOTIFY-03E`

- bounded `NotificationFanoutWorker`, default/hard batch 32/64 and audience page
  256;
- tenant-scoped source and job work projections preserve the owner boundary;
- pending, due retryable, and expired leased records are selected in stable
  `created_at/id` order without acquiring a lease;
- every source and job delegates to `NotificationFanoutService`, which remains the
  only claim/materialization/page-persistence authority;
- default-off host flag `RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED`;
- startup requires a background-worker host, materialized non-empty source
  registry, and module registry;
- `EffectiveModulePolicyService::is_enabled` is checked for `notifications` before
  every provider materialization and audience resolution; disabled/error policy
  fails closed;
- shared shutdown is checked between source records and jobs;
- SQLite evidence covers bounded multi-poll source/job processing, four pending
  candidates, and zero notification/delivery rows;
- contract `contracts/notifications-fanout-worker.json` and verifier
  `scripts/verify/verify-notifications-fanout-worker.mjs`.

## Remaining `NOTIFY-01`

- promote module-local migrations into verified global server migration
  composition;
- retention, reconciliation, repair, quarantine replay/purge, and administrative
  command state;
- keep inbox, preference, digest, and delivery transports closed until matching
  owner commands exist.

## Remaining `NOTIFY-03`

- durable backoff or suppression policy for work belonging to disabled tenants;
- grouping policy and bounded moderator-directory expansion;
- channel work enqueue only after candidate policy acceptance;
- PostgreSQL lease/contention/retry evidence for intake, fanout, and candidates;
- worker health, queue lag, retry, and quarantine metrics before default
  deployment enablement.

## Remaining `NOTIFY-07`

- tenant restrictions beyond effective module capability;
- block/mute management transports and relation change events;
- privacy and source rechecks on inbox open and delayed delivery;
- redaction/archive reconciliation after source/profile changes;
- executable blocked/private/deleted and cross-tenant evidence.

## UI gate

Admin and storefront remain module-owned. Until inbox APIs exist, they expose
only foundation/unavailable states and must not invent unread counts or shadow
inbox storage.

## Maintainer verification set

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --all-targets
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_worker_sqlite -- --nocapture
cargo test -p rustok-notifications --test outbox_intake_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_worker_sqlite -- --nocapture
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

These commands were not executed while publishing the `NOTIFY-03D/03E` source
slices. `Cargo.lock` was not regenerated because the owner dependency set was
restored to the already locked package graph.

## Update rules

- keep canonical program status in the cross-module plan;
- never move producer subscriptions, contact data, or channel SDKs into this
  owner;
- never add synchronous notification calls to producer transactions;
- never create final notification rows before preference/privacy/source policy;
- never create channel delivery work in candidate finalization;
- add persistence or UI behavior only with matching contracts, migrations,
  degraded-mode notes, and verification commands.
