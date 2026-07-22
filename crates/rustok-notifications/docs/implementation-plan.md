# `rustok-notifications` module-local implementation gates

The canonical cross-module task order and status remain in
`crates/rustok-forum/docs/implementation-plan.md`. This file does not duplicate
that backlog; it records the owner-local gates that future notification slices
must preserve. `NOTIFY-00`, `NOTIFY-01`, and `NOTIFY-03` remain `in_progress`
until maintainer-run verification and canonical-plan promotion are recorded.

## Scope

Preserve the neutral producer boundary, owner-only persistence, optional-module
degraded behavior, and module-owned UI packages while the notifications product
is implemented incrementally.

## Current State

The neutral API, bounded source/provider registries, deferred host factory
materialization, optional owner composition, and explicit admin/storefront
foundation states exist. Forum publishes live providers for
`forum.topic.created` and `forum.mention.user_added`.

The owner persistence migrations now create notification, delivery-attempt,
fan-out, preference, digest, encrypted push-subscription, and durable source-inbox
storage for PostgreSQL and SQLite. Typed Rust/DB values, tenant-composite recipient
integrity, bounded payloads, stable dedupe/idempotency keys, lease/completion
guards, and source-event conflict detection are enforced.

`NotificationFanoutService` now durably accepts source events, materializes a
bounded provider descriptor into one fan-out job, and persists one cursor page of
idempotent pending candidates under recoverable leases. It deliberately creates
no final notification or delivery row. Preference, profile/block privacy,
recipient-specific source authorization, retention/reconciliation, and all
inbox/delivery APIs remain open.

## Milestones

### Boundary gate

- producer modules depend only on `rustok-notifications-api`;
- producer transactions never call the notifications owner synchronously;
- semantic descriptors contain bounded template data and target identity, not
  contact data or private source payloads;
- audience resolution is cursor-based and capped;
- target opening is reauthorized for the tenant and recipient;
- provider absence and module absence are explicit degraded states.

### Delivered in `NOTIFY-00B`

- `rustok-notifications` is declared in `modules.toml`, distribution features,
  the server, and owner-owned admin/storefront host packages;
- notifications is compiled into the selected server distribution but remains
  outside `settings.default_enabled`;
- source modules register deferred factories before database services exist;
- the executable host materializes factories after constructing
  `HostRuntimeContext`;
- factory/provider slug conflicts and build failures fail startup explicitly;
- Forum commands succeed without the notifications owner;
- the Forum topic-created provider binds source identity to the owner event
  journal, emits bounded template data, pages category watchers, excludes the
  actor, fails closed for channel restrictions, and reauthorizes the current
  tenant/open target;
- the SQLite profile proves notifications-off command success, notifications-on
  provider materialization, bounded cursor paging, cross-tenant/non-open target
  fallback, and retryable database failure classification;
- static runtime fixtures reject default-enabled composition, Forum imports of
  the owner crate, and removal of the channel fail-closed guard.

### Delivered in `NOTIFY-01A`

- module-owned PostgreSQL/SQLite migration
  `m20260721_000010_create_notification_persistence`;
- typed owner entities for notifications, channel delivery attempts, fan-out
  jobs/items, preferences, digest jobs/items, and push subscriptions;
- composite `users(tenant_id,id)` identity and tenant-composite recipient/user
  foreign keys;
- tenant guards for optional actor and fan-out notification references;
- minimum notification dedupe by tenant, recipient, source slug, source event ID,
  and notification type;
- tenant-scoped idempotency keys for notification, delivery, fan-out item, and
  digest item writes;
- database state/channel/priority/mode checks, read-implies-seen, lease and
  completion timestamp invariants;
- 8 KiB notification template data and 16 KiB fan-out descriptor limits plus
  bounded cursors and provider error fields;
- push endpoint material stored only as encrypted values with endpoint hash and
  key version;
- SQLite and opt-in PostgreSQL invariant profiles;
- static fixtures reject missing composite recipient integrity, raw contact or
  source-private fields, and plaintext push endpoint columns.

### Delivered in `NOTIFY-01B/03A`

- module-owned PostgreSQL/SQLite migration
  `m20260722_000011_create_notification_source_inbox` ordered after `NOTIFY-01A`;
- durable source inbox dedupe by tenant/source/event identity with changed
  event-type or source-revision conflict;
- typed pending/processing/completed/suppressed/retryable/rejected states,
  bounded error metadata, retry timing, leases, expired-lease recovery, and a
  retained fan-out job link;
- provider-independent event acceptance followed by leased descriptor
  materialization, so temporary source-factory absence is retryable rather than
  data loss;
- one descriptor-bound fan-out job per source event and notification type with
  replay equality checks;
- bounded cursor pages capped at 256 and fail-closed non-advancing cursors;
- idempotent pending candidate items deduplicated by tenant/job/recipient;
- no final notification row or delivery attempt before preference/privacy policy;
- Forum `forum.mention.user_added` provider bound to the exact immutable relation
  row, with source visibility checks at describe/audience/open time, retryable
  pending replies, self-mention suppression, and closed/deleted/hidden/channel
  fail-closed behavior;
- SQLite owner scenarios for source replay/conflict, two-page fan-out, terminal
  replay, zero notification rows, and Forum user-mention provider behavior;
- machine contract and source verifier under
  `contracts/notifications-source-fanout.json` and
  `scripts/verify/verify-notifications-source-fanout.mjs`.

### Remaining `NOTIFY-01` scope

- global server migrator registration after maintainer verification of the
  module-local PostgreSQL/SQLite schema;
- final notification persistence commands after preference/privacy decisions;
- explicit retention and reconciliation metadata/commands;
- inbox, preference, digest, and delivery transport APIs remain closed until the
  owner command semantics are implemented.

### Remaining `NOTIFY-03` scope

- wire production outbox relay consumption into `enqueue_source_event`;
- process pending candidates through preference, block/profile privacy, source
  authorization, dedupe/grouping, notification creation, and channel enqueue;
- add bounded moderator-audience expansion through an owner directory port;
- add PostgreSQL lease/concurrency/retry runtime evidence and DLQ/replay controls.

### UI gate

Admin and storefront packages remain module-owned. Until inbox APIs exist, they
expose only bootstrap/unavailable states and must not invent unread counts or
store shadow inbox state in the host.

## Verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
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
cargo xtask module validate notifications
```

The commands above are the maintainer verification set. They were not executed
while publishing the `NOTIFY-01B/03A` source slice.

## Update Rules

- keep task status in the canonical forum/notifications program plan;
- update this file only for owner-local gates or verified runtime shape;
- never move producer subscriptions, contact data, or channel SDKs into this
  owner;
- never add synchronous notification calls to producer transactions;
- never create final notification or delivery rows before preference/privacy
  policy has accepted a pending candidate;
- add persistence and UI behavior only with matching owner contracts, migrations,
  degraded-mode notes, and verification commands.
