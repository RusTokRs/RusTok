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
- candidate worker enablement remains a separate explicit false gate.

Privacy reads remain authoritative when tenant-facing Social Graph surfaces are
not enabled: disabling management UX must not silently bypass an already stored
block or mute.

## Remaining Social Graph scope

- durable command receipts that bind idempotency keys to command identity;
- friendship request/accept/remove lifecycle;
- follow/unfollow and follower privacy;
- custom lists and list membership;
- commands/transports for block and mute management;
- outbox events and reconciliation;
- moderation/admin repair commands;
- PostgreSQL concurrency evidence and retention policy.

## Remaining Notifications scope

- production outbox relay consumption;
- production candidate worker startup after runtime readiness;
- inbox-open and delayed-delivery privacy rechecks;
- grouping, moderator expansion, channel delivery, and retention/reconciliation.

## Verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --all-targets
cargo test -p rustok-social-graph --test privacy_sqlite -- --nocapture
node scripts/verify/verify-social-graph-notification-policy.mjs
```

These commands are maintainer-run and were not executed while publishing this
slice. `Cargo.lock` was not regenerated because Cargo was not run.
