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

- durable command receipts that bind idempotency keys to command identity;
- friendship request/accept/remove lifecycle;
- broader profile directory/follow product UX beyond the first storefront profile page;
- custom lists and list membership;
- commands/transports for block and mute management;
- outbox events and reconciliation;
- moderation/admin repair commands;
- PostgreSQL concurrency evidence and retention policy.

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
node scripts/verify/verify-social-graph-notification-policy.mjs
node scripts/verify/verify-profiles-storefront-boundary.mjs
node scripts/verify/verify-notifications-candidate-worker.mjs
node scripts/verify/verify-notifications-outbox-intake.mjs
```

These commands are maintainer-run and were not executed while publishing this
slice. `Cargo.lock` was not regenerated because Cargo was not run.
