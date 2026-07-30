# rustok-social-graph

Tenant-scoped owner for social relationships and privacy-relevant relation state.

The executable surface owns `block`, `mute`, and directional `follow` state.
Friendship, lists, recommendations, block/mute management transports, and product
UI remain deferred until matching owner contracts are implemented.

## Interactions

- depends on platform tenant/user identity only through migration ordering and
  tenant-composite foreign keys;
- declares `outbox` in both root and module manifests because relation writes
  require transactional `sys_events` persistence;
- exposes neutral command, privacy read, revision-bearing follow-state read,
  owner-only receipt maintenance, and bounded relation-event replay ports from
  this crate;
- exposes an optional module-owned GraphQL follow transport through the `graphql`
  feature;
- exposes owner-local operational CLI composition through
  `rustok-social-graph-cli`; the adapter delegates to owner maintenance ports and
  never reads receipt or relation storage directly;
- does not depend on Profiles, Notifications, or read foreign-domain persistence;
- the server and other modules may adapt owner ports into consumer-specific runtime ports;
- block is strict and symmetric for privacy evaluation when either direction is active;
- mute is directional from the muting user to the hidden user;
- follow is directional from follower (`source_user_id`) to profile owner
  (`target_user_id`);
- follow presentation reads support one bounded batch of at most 100 target users,
  deduplicate target ids, and return only active relations;
- `SocialGraphFollowReadPort` returns active state plus the optional current revision for one owned source/target pair, including inactive persisted relations;
- user port actors may read or mutate only relation sources they own, while
  service/system actors remain available for owner composition;
- GraphQL `isFollowing`, `followState`, `followUser`, and `unfollowUser` are human-user-only,
  tenant-bound, and call owner ports with deadline and idempotency semantics;
- GraphQL optimistic revisions are represented as positive 64-bit integer strings;
- `SocialGraphCommandPort` persists a versioned owner-private command receipt in the
  same transaction as each committed relation state change;
- receipt identity is unique by tenant and normalized idempotency key, while the
  stored request payload binds source, target, relation kind, requested state, and
  expected revision;
- exact replay returns the original relation response snapshot without rewinding
  newer live relation state; reuse of a key for a different command returns
  `social_graph.idempotency_conflict`;
- receipt payloads are limited to schema version `1`, processing/completed state is
  constrained in PostgreSQL and SQLite, and corrupt/incomplete records fail closed;
- a real relation insert or active-state transition publishes sealed typed event
  `social_graph.relation.state_changed` through `TransactionalEventBus` before the
  receipt is completed and the owner transaction commits;
- the event is an authoritative persisted-revision fact. It carries only relation
  id, source/target user ids, canonical kind, active state, and revision; tenant and
  actor remain envelope metadata, while command idempotency, expected revision,
  request context, and receipt snapshots are excluded;
- receipt replay and exact persisted-state no-op complete without another live event;
  publication failure rolls back relation state and the receipt together and returns
  `social_graph.event_publication_unavailable`;
- GraphQL and Profiles native storefront writes require the host-composed
  `TransactionalEventBus`; read-only Social Graph composition remains DB-only;
- `SocialGraphRelationEventMaintenancePort` is implemented by a separate owner
  service. It accepts only service/system actors with `event_replay` policy and
  selects one tenant-scoped page of at most 1000 persisted relations in stable UUID
  cursor order;
- bounded relation-event replay is enabled only after all active writers use the
  atomic live event path. The UUID cursor therefore covers the fixed historical
  backlog; concurrent new writes already publish their own event;
- dry-run returns selected count and the next UUID cursor without writing Outbox;
  a live page appends every selected event in one transaction, so one failed insert
  rolls the whole page back;
- replay is at-least-once and may repeat the same relation revision. An approved
  consumer must keep Social Graph authoritative and apply events by relation id plus
  monotonic revision, ignoring duplicate or lower revisions before acknowledging;
- replay telemetry is aggregate only: tenant, dry-run mode, limit, cursor presence,
  selected/published counts, duration, outcome, stable code, and retryability. Raw
  cursor values and per-relation identifiers are not logged;
- `rustok-cli social_graph relation-event-replay` requires explicit `--tenant-id`,
  accepts only an optional UUID `--after-relation-id` and bounded `--limit` from 1 to
  1000, defaults the limit to 100, and supports `--dry-run`;
- the replay CLI uses the owner-composed transactional outbox, processes exactly one
  page per invocation, and returns `next_after_relation_id` for explicit continuation;
- [relation-event replay CLI contract](./docs/relation-event-replay-cli.md) documents
  the operator boundary and non-claims;
- `SocialGraphReceiptMaintenancePort` is implemented by a separate owner service,
  accepts only service/system actors with write-port policy, and supports explicit
  tenant-scoped dry-run or deletion of at most 1000 completed receipts before a
  caller-selected Unix-time cutoff;
- the cutoff must be a valid timestamp strictly in the past; a future cutoff fails
  with `social_graph.receipt_cleanup_cutoff_future` instead of risking deletion of
  the current replay window;
- cleanup selects schema-v1 completed rows with non-null completion time in stable
  completion/id order, then validates every candidate request/response snapshot
  before deleting any row; one corrupt candidate aborts the whole batch;
- processing, corrupt, another-tenant, and in-window rows are never deleted;
- `rustok-cli social_graph receipt-cleanup` requires explicit `--tenant-id` and
  positive `--retention-days`, derives the cutoff inside the owner-local adapter,
  accepts only a bounded `--limit` from 1 to 1000, and supports `--dry-run`;
- the CLI has no deployment retention default. Its output is aggregate only and it
  calls `SocialGraphReceiptMaintenancePort` rather than owner-private tables;
- cleanup is not scheduled automatically: deployment-specific replay windows,
  cadence, and rollout evidence remain operator-owned;
- `SocialGraphCommandPort` emits stable owner-operation telemetry for block/unblock,
  mute/unmute, and follow/unfollow through `rustok_social_graph::operations`;
- cleanup emits one aggregate `social_graph.receipt_cleanup` operation for policy,
  actor, validation, storage, and success outcomes with tenant, dry-run mode, limit,
  matched/deleted counts, oldest retained completion time, bounded duration, and
  stable error classification; keys and receipt payloads remain excluded;
- command telemetry records only operation, tenant/source/target identifiers,
  outcome, bounded duration, stable error code, and retryability; it does not record
  idempotency keys, expected revisions, locale, channel, claims, roles, or correlation ids;
- missing/error owner state must not be converted into implicit allow.

## Verification

```bash
cargo check -p rustok-events --all-targets
cargo test -p rustok-events --test social_graph_contracts -- --nocapture
cargo run -p rustok-events --example event_contract_digests
cargo check -p rustok-social-graph --all-targets
cargo check -p rustok-social-graph --features graphql --all-targets
cargo check -p rustok-social-graph-cli --all-targets
cargo test -p rustok-social-graph-cli -- --nocapture
cargo test -p rustok-social-graph --test privacy_sqlite -- --nocapture
cargo test -p rustok-social-graph --test follow_sqlite -- --nocapture
cargo test -p rustok-social-graph --test follow_state_sqlite -- --nocapture
cargo test -p rustok-social-graph --test command_receipts_sqlite -- --nocapture
cargo test -p rustok-social-graph --test receipt_cleanup_sqlite -- --nocapture
cargo test -p rustok-social-graph --test relation_outbox_sqlite -- --nocapture
cargo test -p rustok-social-graph --test relation_event_replay_sqlite -- --nocapture
node scripts/generate/generate-cli-registry.mjs --check
node scripts/verify/verify-social-graph-notification-policy.mjs
node scripts/verify/verify-social-graph-command-receipts.mjs
node scripts/verify/verify-social-graph-receipt-cleanup.mjs
node scripts/verify/verify-social-graph-receipt-cleanup-cli.mjs
node scripts/verify/verify-social-graph-relation-outbox.mjs
node scripts/verify/verify-social-graph-relation-event-replay.mjs
node scripts/verify/verify-profiles-storefront-boundary.mjs
rustok-cli social_graph receipt-cleanup --tenant-id <uuid> --retention-days 30 --limit 100 --dry-run
rustok-cli social_graph relation-event-replay --tenant-id <uuid> --limit 100 --dry-run
```
