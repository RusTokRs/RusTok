# `rustok-social-graph` implementation gates

The social graph owner is introduced by `SOCIAL-01A / NOTIFY-07C`. The canonical
cross-module roadmap remains `crates/rustok-forum/docs/implementation-plan.md`.

## Delivered in `SOCIAL-01A / NOTIFY-07C`

- PostgreSQL and SQLite migration
  `m20260723_000001_create_social_graph_relations`;
- one tenant-scoped identity row per source user, target user, and relation kind;
- current `block` and `mute` state with monotonic revision and semantic state
  replay;
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
- `SocialRelationKind::Follow` uses directional follower
  (`source_user_id`) to profile owner (`target_user_id`) semantics;
- the existing owner command port persists follow/unfollow state with the same
  tenant-composite integrity, replay, revision, actor, deadline, and idempotency
  rules as block/mute;
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
- the Social Graph owner remains independent from Profiles presentation storage
  and does not read profile rows, translations, tags, or media.

## Delivered public follow transport

- optional crate feature `graphql` exposes module-owned `SocialGraphQuery` and
  `SocialGraphMutation` roots through `rustok-module.toml`;
- `isFollowing(userId)` reads only the authenticated human user's directional
  active state and returns `false` for self without creating an existence oracle;
- `followState(userId)` returns target user, active state, and optional revision
  string without exposing relation ids or storage details;
- `followUser` and `unfollowUser` require explicit `idempotencyKey`, accept an
  optional positive 64-bit `expectedRevision` string, and delegate to
  `SocialGraphCommandPort`;
- transport context is tenant-bound, human-user-only, deadline-aware, and carries
  authenticated permission claims and optional channel context;
- service principals and tenant mismatches are rejected before owner calls;
- mutation responses expose only target user, active state, and revision string;
  internal relation ids and storage details remain private;
- validation/conflict/forbidden semantics remain typed while unavailable and
  invariant failures use static public GraphQL messages;
- the server enables the Social Graph GraphQL feature while non-host consumers may
  keep the transport disabled.

## Delivered owner-operation telemetry

- `SocialGraphCommandPort` owns the telemetry boundary, so GraphQL and future
  adapters cannot drift into transport-local instrumentation;
- one command record is emitted for block/unblock, mute/unmute, and
  follow/unfollow through the stable `rustok_social_graph::operations` target;
- records contain only operation, tenant/source/target UUIDs, success/failure,
  bounded duration, stable `PortError.code`, and retryability;
- missing idempotency/deadline policy and source-actor failures are recorded after
  tenant parsing, while owner validation/conflict/storage results use the same
  result classifier;
- idempotency keys, expected revisions, request correlation, locale, channel,
  claims, and roles are explicitly excluded;
- the Profiles storefront source verifier locks operation names, fields,
  exclusions, owner-port placement, and the absence of transport-local tracing.

## Delivered durable command receipts

- migration `m20260726_000003_create_command_receipts` owns a PostgreSQL/SQLite
  `social_graph_command_receipts` table with tenant-scoped unique idempotency
  identity, bounded keys, versioned JSON payloads, processing/completed state, and
  completion-integrity checks;
- the owner command port normalizes keys to 1..191 bytes and admits receipts before
  mutating relation state;
- receipt reservation, relation mutation, response snapshot, and completion commit
  share one database transaction;
- an exact replay returns the original relation response snapshot even when a later
  command has advanced the live relation revision;
- reusing a key with a different source/target/kind/state/expected-revision payload
  fails with `social_graph.idempotency_conflict` and does not mutate relation state;
- unsupported receipt schema versions and incomplete/corrupt records fail closed as
  `social_graph.command_receipt_corrupt`;
- raw idempotency keys and receipt payloads remain excluded from operation telemetry;
- a dedicated source verifier locks storage constraints, owner-private placement,
  transactional integration, stable error codes, telemetry exclusions, and the
  SQLite replay/conflict scenario.

## Promoted by `NOTIFY-03C`

- the production candidate worker consumes Social Graph only through the existing
  notification policy adapters;
- worker runtime and graceful shutdown are delivered;
- startup remains disabled by default and requires the explicit
  `RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED` flag plus ready relation ports;
- this promotion does not add notification-specific behavior to the Social Graph
  owner or expose its private tables.

## Promoted by `NOTIFY-03D`

- Notifications accepts supported committed outbox envelopes into its own
  durable source inbox and intake-receipt state;
- intake remains independent from Social Graph and does not evaluate recipient
  privacy before fan-out candidates exist;
- Social Graph is still consulted only by the candidate policy after source
  materialization and audience expansion;
- the intake runtime is separately default-off behind
  `RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED`.

## Remaining Social Graph scope

- relation outbox events and reconciliation;
- receipt retention/cleanup policy and PostgreSQL concurrency evidence;
- friendship request/accept/remove lifecycle;
- broader profile directory/follow product UX beyond the first storefront profile page;
- custom lists and list membership;
- commands/transports for block and mute management;
- moderation/admin repair commands;
- retained runtime evidence for command receipt replay/conflict and telemetry success,
  conflict, policy, actor-mismatch, and storage-unavailable outcomes.

## Remaining Notifications scope

- production source-inbox materialization and bounded fan-out page worker;
- tenant capability enforcement before materialization and delivery;
- default worker enablement after health and queue-lag metrics;
- inbox-open and delayed-delivery privacy rechecks;
- grouping, moderator expansion, channel delivery, and retention/reconciliation.

## Verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features graphql --all-targets
cargo test -p rustok-social-graph --test privacy_sqlite -- --nocapture
cargo test -p rustok-social-graph --test follow_sqlite -- --nocapture
cargo test -p rustok-social-graph --test follow_state_sqlite -- --nocapture
cargo test -p rustok-social-graph --test command_receipts_sqlite -- --nocapture
node scripts/verify/verify-social-graph-notification-policy.mjs
node scripts/verify/verify-social-graph-command-receipts.mjs
node scripts/verify/verify-profiles-storefront-boundary.mjs
node scripts/verify/verify-notifications-candidate-worker.mjs
node scripts/verify/verify-notifications-outbox-intake.mjs
```

These commands are maintainer-run and were not executed while publishing this
slice. `Cargo.lock` was not regenerated because Cargo was not run.
