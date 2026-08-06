---
id: doc://crates/rustok-reactions/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-reactions
last_reviewed: 2026-08-06
---

# `rustok-reactions` implementation plan

## Program ledger

| Task | Status | Deliverable |
| --- | --- | --- |
| `REACTIONS-00` | `in_progress` | Neutral API, optional owner module, distribution/server feature selection and provider registries are source-ready; maintainer Cargo/Node verification remains. |
| `REACTIONS-01` | `in_progress` | Tenant-composite owner schema, immutable catalog snapshots, actor uniqueness and shared Outbox command receipts are source-ready. Regenerate `Cargo.lock` and retain SQLite/PostgreSQL execution evidence. |
| `REACTIONS-02` | `in_progress` | Actor state and aggregate deltas commit atomically behind one subject serialization row. Add semantic events, repair/reconciliation and retained concurrency evidence. |
| `REACTIONS-03` | `in_progress` | Forum topic/reply provider, optional host materialization and executable composition profile tests are source-ready. Reactions stays outside defaults; retained execution evidence remains pending. |
| `REACTIONS-04` | `planned` | Second real producer adapter and neutral-contract review. |
| `REACTIONS-05` | `planned` | Bounded read/write transports and module-owned UI. |
| `REACTIONS-06` | `planned` | Runtime evidence, FBA contracts, import/reconciliation and release profiles. |

## Ownership

Reactions owns catalog snapshots, actor state, shared-receipt-bound command
execution and aggregate projections. Producer modules own subject existence,
current revision, visibility, lifecycle and reaction policy. Profiles owns actor
presentation. Reputation and achievements consume semantic facts but do not
mutate reaction state. Notifications may consume future events but are not part
of command correctness.

## Persistence invariants

- Every row is tenant-scoped.
- Subject identity is `(tenant, source, kind, subject UUID)`.
- Subject and catalog revisions are positive and monotonic.
- One catalog revision is immutably bound to one catalog payload.
- One actor state exists per tenant/subject/actor.
- Selection keys remain inside the authorized bounded catalog.
- Aggregate counts never become negative and change atomically with actor state.
- Command UUID equals the shared Outbox idempotency key.
- Producer authorization happens before owner storage access.
- Reactions never reads producer-private tables.

The initial catalog revision is the producer subject revision. Independent
catalog revisioning is a later explicit API/migration change.

## Forum producer boundary

Forum registers a neutral `ReactionSubjectProviderFactory` for `topic` and
`reply` without depending on the Reactions owner. The provider:

- validates exact tenant/source/kind identity;
- hides missing, deleted, lifecycle-denied and audience-denied subjects behind
  one unavailable result;
- checks existing Forum rich-audience visibility before returning revision
  conflicts;
- uses `latest captured Forum revision id + 1` as the current subject revision;
- allows only approved replies whose parent topic is currently open and visible;
- publishes one bounded single-selection `like` catalog for the initial contract;
- resolves delegated actors through the existing exact recipient-context port;
- does not read or reinterpret existing Forum vote state.

Forum remains fully usable when Reactions is absent. The neutral factory may be
registered without materializing a Reactions owner runtime.

## Optional host composition boundary

`mod-reactions` is an explicit optional feature in `rustok-distribution` and
`rustok-server`. It registers `ReactionsModule`, contributes the owner migration
source and materializes the reaction subject registry only after host providers
have been composed. The optional distribution/server host materialization is
source-ready. The enabled/disabled runtime evidence now has executable test
source, but retained runs remain pending.

The executable host fails closed when the feature is selected but
`ReactionsModule` is absent from the supplied `ModuleRegistry`. When Forum is
also selected, materialization happens after Forum audience facts and exact
recipient-context providers exist, so the Forum factory cannot be built with a
broader or incomplete authority boundary.

Supported profiles are explicit:

- Forum without Reactions: Forum remains available; the neutral factory is not
  materialized into an owner registry.
- Reactions without Forum: the owner registry materializes with no Forum source.
- Forum with Reactions: the registry materializes the `forum` source with
  `topic` and `reply` kinds.
- Selected Reactions feature without `ReactionsModule`: startup fails with the
  stable owner-missing error before publishing a false runtime.

`mod-forum` does not imply `mod-reactions`, server defaults do not select it and
`modules.toml` keeps Reactions outside `default_enabled`.

## Executable composition evidence

`apps/server/tests/reactions_composition_profiles.rs` provides executable
composition profile tests over the public server host-composition entrypoint and
an isolated SQLite in-memory connection. The test target proves the three
supported optional profiles plus the selected-feature/missing-owner failure.
It does not query Forum domain rows or execute reaction commands.

The contract remains `source_ready_maintainer_execution_pending`: retained
execution evidence remains pending until a maintainer runs and stores the four
profile results. Source presence alone does not promote `REACTIONS-03` to done.

## Immediate next action

After retaining the four composition-profile runs and regenerating `Cargo.lock`,
add transactional semantic reaction events and bounded catalog/aggregate
reconciliation. Then add a second real producer before freezing transport or
presentation contracts.

Before release, execute SQLite and PostgreSQL migrations, retain replay,
concurrency and rollback evidence, and provide reconciliation for catalog and
aggregate drift.

## Verification

```bash
cargo test -p rustok-reactions-api
cargo test -p rustok-reactions
cargo test -p rustok-forum reaction_subject
cargo check -p rustok-reactions-api --all-targets
cargo check -p rustok-reactions --all-targets
cargo check -p rustok-forum --all-targets
cargo check -p rustok-distribution --features "mod-forum mod-reactions"
cargo test -p rustok-server --no-default-features --features mod-forum --test reactions_composition_profiles forum_without_reactions_keeps_forum_host_composition_available
cargo test -p rustok-server --no-default-features --features mod-reactions --test reactions_composition_profiles reactions_without_forum_materializes_an_empty_subject_registry
cargo test -p rustok-server --no-default-features --features mod-reactions --test reactions_composition_profiles selected_reactions_feature_fails_when_owner_module_is_missing
cargo test -p rustok-server --no-default-features --features "mod-forum mod-reactions" --test reactions_composition_profiles forum_with_reactions_materializes_topic_and_reply_provider
node scripts/verify/verify-reactions-foundation.mjs
node scripts/verify/verify-reactions-owner-persistence.mjs
node scripts/verify/verify-forum-reaction-subject-provider.mjs
node scripts/verify/verify-reactions-host-composition.mjs
node scripts/verify/verify-reactions-composition-profiles.mjs
git diff --check
```

Tests, checks, lockfile generation and retained runtime evidence are maintainer-run.
