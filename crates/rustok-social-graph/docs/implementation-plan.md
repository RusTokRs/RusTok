# `rustok-social-graph` implementation gates

The social graph owner is introduced by `SOCIAL-01A / NOTIFY-07C`. The canonical
cross-module roadmap remains `crates/rustok-forum/docs/implementation-plan.md`.

## Delivered in `SOCIAL-01A / NOTIFY-07C`

- PostgreSQL and SQLite migration
  `m20260723_000001_create_social_graph_relations`;
- one tenant-scoped identity row per source user, target user, and relation kind;
- current `block` and `mute` state with monotonic revision and semantic state replay;
- tenant-composite foreign keys for both users and self-relation rejection;
- owner command port with deadline, required idempotency-key presence,
  source-actor, and optional expected-revision gates;
- neutral `SocialGraphPrivacyReadPort` for symmetric block and directional mute
  evaluation;
- server-owned adapters into Notifications block/mute runtime contracts;
- notification recipient relation-policy readiness is true with both concrete
  owner adapters;
- candidate worker enablement remains a separate explicit gate and is false by
  default.

Privacy reads remain authoritative when tenant-facing Social Graph surfaces are
not enabled: disabling management UX must not silently bypass an already stored
block or mute.

## Delivered for Profiles follower policy

- migration `m20260725_000002_add_follow_relation_kind` expands the PostgreSQL and
  SQLite relation-kind constraint while preserving existing block/mute rows and
  indexes;
- `SocialRelationKind::Follow` uses directional follower (`source_user_id`) to
  profile owner (`target_user_id`) semantics;
- the owner command port persists follow/unfollow with the same tenant-composite
  integrity, revision, actor, deadline, and idempotency rules as block/mute;
- `SocialGraphPrivacyReadPort` exposes directional single-target and bounded
  multi-target active follow reads;
- `SocialGraphFollowReadPort` exposes one actor-bound source/target state with
  active flag and optional current revision, including inactive persisted rows;
- follow batches accept at most 100 target users, deduplicate input, return only
  active followed targets, and reject user actors that do not own the source;
- `rustok-profiles` composes followers-only visibility through the owner port in
  bounded chunks and propagates owner errors instead of allowing implicitly;
- the Profiles storefront uses the revision-bearing read for initial state and
  read-only conflict recovery without automatic command replay;
- the Social Graph owner remains independent from Profiles presentation storage.

## Delivered public follow transport

- optional crate feature `graphql` exposes module-owned `SocialGraphQuery` and
  `SocialGraphMutation` roots through `rustok-module.toml`;
- `isFollowing(userId)` and `followState(userId)` read only the authenticated
  human user's directional state and do not expose relation ids;
- `followUser` and `unfollowUser` require explicit `idempotencyKey`, accept an
  optional positive 64-bit `expectedRevision`, and delegate to
  `SocialGraphCommandPort`;
- transport context is tenant-bound, human-user-only, deadline-aware, and carries
  authenticated permission claims and optional channel context;
- service principals and tenant mismatches are rejected before owner calls;
- validation/conflict/forbidden semantics remain typed while unavailable and
  invariant failures use static public GraphQL messages.

## Delivered owner-operation telemetry

- `SocialGraphCommandPort` owns the telemetry boundary, so GraphQL and operational
  adapters cannot drift into transport-local instrumentation;
- one command record is emitted for block/unblock, mute/unmute, and follow/unfollow
  through the stable `rustok_social_graph::operations` target;
- records contain only operation, tenant/source/target UUIDs, outcome, bounded
  duration, stable `PortError.code`, and retryability;
- idempotency keys, expected revisions, request correlation, locale, channel,
  claims, and roles are explicitly excluded.

## Delivered durable command receipts

- migration `m20260726_000003_create_command_receipts` owns PostgreSQL/SQLite
  `social_graph_command_receipts` with tenant-scoped unique idempotency identity,
  bounded keys, versioned JSON payloads, processing/completed state, and
  completion-integrity checks;
- the owner command port normalizes keys to 1..191 bytes and admits receipts before
  mutating relation state;
- receipt reservation, relation mutation, response snapshot, and completion commit
  share one database transaction;
- exact replay returns the original relation response snapshot even when a later
  command has advanced live revision;
- reusing a key with another source/target/kind/state/expected-revision payload
  fails with `social_graph.idempotency_conflict` and does not mutate relation state;
- unsupported receipt schemas and incomplete/corrupt records fail closed as
  `social_graph.command_receipt_corrupt`;
- raw idempotency keys and receipt payloads remain excluded from operation telemetry;
- the migration includes a tenant/status/completion/id cleanup index.

## Delivered bounded receipt maintenance

- `SocialGraphReceiptMaintenancePort` is implemented by
  `SocialGraphReceiptMaintenanceService` over the owner database connection;
- callers require write-port deadline/idempotency semantics and only service/system
  actors are accepted;
- commands carry an explicit Unix cutoff, dry-run mode, and limit from 1 to 1000;
- selection is tenant-scoped, schema-v1, `completed` only, requires non-null
  completion time, applies a strict cutoff, and orders by `(completed_at, id)`;
- every candidate request/response snapshot is validated before any delete;
- deletion repeats tenant/schema/status/cutoff predicates and selected ids only;
- processing, corrupt, another-tenant, and in-window rows are never candidates;
- results report matched/deleted counts and oldest retained completion time;
- aggregate telemetry excludes raw keys and payloads;
- SQLite evidence covers dry-run, bounded deletion, retained-floor reporting,
  processing preservation, tenant isolation, user denial, future cutoff, invalid
  limit, and all-or-nothing corrupt-candidate failure.

## Delivered owner-local receipt cleanup CLI

- direct workspace crate `rustok-social-graph-cli` implements the selected
  distribution `CommandProvider` for `social_graph receipt-cleanup`;
- `rustok-module.toml` declares `[provides.cli]`, while the generated CLI registry
  composes `rustok_social_graph_cli::command_provider` from `RuntimeComposition`;
- `--tenant-id` and positive `--retention-days` are mandatory; there is no
  deployment retention default;
- the adapter derives `completed_before_unix_seconds` as `now - retention_days`
  and delegates to `SocialGraphReceiptMaintenancePort`;
- `--limit` defaults only the batch size to 100 and remains bounded by the owner
  maximum of 1000;
- `--dry-run` selects through the same owner path without deletion;
- the context uses a system actor, bounded deadline, and operation idempotency
  identity derived from tenant, cutoff, limit, and mode;
- output is aggregate only: tenant, retention window, cutoff, mode, limit,
  matched/deleted counts, and oldest retained completion time;
- the adapter does not import receipt entities, read receipt tables, schedule
  execution, or introduce an automatic worker;
- `docs/receipt-cleanup-cli.md` owns rollout and rollback guidance;
- `verify-social-graph-receipt-cleanup-cli.mjs` locks workspace, manifest, registry,
  owner-port delegation, required retention input, bounds, safe output, and absence
  of scheduler/direct-storage behavior.

## Delivered transactional relation events

- `rustok-events` owns sealed typed contract
  `social_graph.relation.state_changed` schema version 1;
- payload contains relation id, source/target user ids, canonical relation kind,
  active state, and revision only; tenant and actor remain envelope metadata;
- command idempotency, expected revision, request context, receipt snapshots,
  claims, roles, locale, and channel are excluded;
- `SocialGraphService::with_event_bus` is explicit write composition while
  `SocialGraphService::new` remains read-only composition;
- new relation or active-state transition publishes through
  `TransactionalEventBus::publish_contract_in_tx` before receipt completion and
  shared transaction commit;
- receipt replay and exact persisted-state no-op publish no new live event;
- event failure rolls relation and receipt back together and returns
  `social_graph.event_publication_unavailable`;
- GraphQL and Profiles native storefront writes require the host transactional bus;
- SQLite evidence covers create/update payload, no-op/replay suppression, and
  rollback when `sys_events` is absent.

## Delivered bounded relation-event replay

- `SocialGraphRelationEventMaintenancePort` is implemented by a separate service
  with explicitly supplied `TransactionalEventBus`;
- callers require `PortCallPolicy::event_replay()` and service/system actor;
- command carries optional exclusive relation UUID cursor, dry-run, and limit 1..1000;
- selection is tenant-scoped, ordered by UUID, applies `id > cursor`, and returns
  selected/published counts plus the last selected UUID;
- dry-run commits no Outbox rows; a live page publishes all selected authoritative
  relations in one transaction and one append failure rolls the page back;
- replay starts only after every active writer uses the atomic live path, so the
  UUID scan covers fixed historical backlog while new writes emit live events;
- replay is at-least-once; consumers must apply by relation id plus monotonic
  revision, ignore duplicate/lower revisions, persist their own result, and
  acknowledge only after durable application;
- Social Graph storage remains authoritative and replay creates no consumer projection;
- telemetry is aggregate and excludes raw cursor/per-relation ids;
- SQLite evidence covers dry-run, tenant isolation, cursor paging, guards, and
  all-or-nothing second-insert failure.

## Delivered first approved Index consumer contract

- `rustok-index` is the first named approved consumer for generic relation
  projection supporting future profile discovery/search and other bounded queries;
- optional feature `index` keeps the consumer adapter out of Social Graph runtime
  composition unless explicitly selected;
- `social_graph_relation_index_schema()` declares a non-localized relation schema
  with source user, target user, and canonical relation kind fields;
- `social_graph_relation_index_mutation(...)` accepts the sealed validated event,
  non-nil tenant/event identity, and uses relation id as Index entity identity;
- active state maps to an upsert and inactive state maps to a tombstone;
- positive relation revision maps exactly to Index `source_version`, enabling the
  Index inbox/mutation store to terminally deduplicate exact delivery and ignore
  lower revisions;
- the adapter reads no Social Graph tables, contains no broker logic, and cannot be
  used for privacy authorization;
- the durable consumer must register the schema, apply/recognize the Index result,
  acknowledge only after commit, and repair drift through bounded Social Graph
  replay or authoritative rescan.

## Receipt retention and rollout contract

- Receipts are externally observable idempotency state, not an expendable cache.
- The deployment retention window must cover the longest supported client retry
  horizon plus clock skew and incident replay allowance.
- The owner-local CLI requires the window explicitly and derives the cutoff; it
  does not provide a retention default.
- Automatic cleanup remains disabled. Deployment cadence or a future worker needs
  separate reviewed configuration and retained PostgreSQL/runtime evidence.
- Run one-tenant replay/conflict evidence, then CLI dry-run, review the retained
  floor, and only then execute bounded live batches.
- Application rollback retains receipt tables and rows and pauses cleanup.
- Unsupported schema, incomplete row, or unexpected processing state remains
  fail-closed and requires operator review.

## Promoted by Notifications work

- production candidate workers consume Social Graph only through existing policy
  adapters and remain separately gated;
- Notifications supported outbox intake is independent from Social Graph and does
  not move owner relation state into Notifications;
- candidate and intake workers keep their own default-off enablement and recovery
  contracts.

## Remaining Social Graph scope

- compose the approved Index consumer group, schema registration, durable
  apply/terminal-recognition, result-first broker acknowledgement, and DLQ policy;
- prove projection drift repair against bounded owner replay/rescan and show that
  Profiles privacy continues to use authoritative owner ports;
- configure deployment retention window/cadence and collect CLI live evidence;
- collect PostgreSQL receipt/event concurrency, cleanup, replay-window, retention,
  bounded replay, relay, Index apply/ack, and rollback evidence;
- friendship request/accept/remove lifecycle;
- broader profile directory/follow UX, custom lists, block/mute management
  transports, and moderation/admin repair commands;
- retained runtime evidence for event relay/replay, receipt replay/conflict/cleanup,
  CLI dry-run/live batches, Index projection repair, and telemetry failure classes.

## Verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-events --all-targets
cargo test -p rustok-events --test social_graph_contracts -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features graphql --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index --all-targets
cargo test -p rustok-social-graph --features index index::tests -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph-cli --all-targets
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
node scripts/verify/verify-social-graph-index-consumer.mjs
node scripts/verify/verify-profiles-storefront-boundary.mjs
rustok-cli social_graph receipt-cleanup --tenant-id <uuid> --retention-days 30 --limit 100 --dry-run
```

These commands remain maintainer-run and were not executed manually while
publishing this slice. `Cargo.lock` must be refreshed by the maintainer because the
new optional workspace edge may change resolved package dependency metadata.
