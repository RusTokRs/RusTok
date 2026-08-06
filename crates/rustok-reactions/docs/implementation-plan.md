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
| `REACTIONS-00` | `in_progress` | Neutral API, optional module registration and provider registry are source-ready; maintainer Cargo/Node verification remains. |
| `REACTIONS-01` | `in_progress` | Tenant-composite owner schema, immutable catalog snapshots, actor uniqueness and shared Outbox command receipts are source-ready. Regenerate `Cargo.lock` and retain SQLite/PostgreSQL execution evidence. |
| `REACTIONS-02` | `in_progress` | Actor state and aggregate deltas commit atomically behind one subject serialization row. Add semantic events, repair/reconciliation and retained concurrency evidence. |
| `REACTIONS-03` | `in_progress` | Forum topic/reply provider factory, exact current-revision/visibility authorization and the Reactions-disabled Forum profile are source-ready. Add optional distribution/host materialization and retain enabled/disabled runtime evidence. |
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

## Immediate next action

Add optional `mod-reactions` selection to `rustok-distribution` and the server,
materialize source factories after host facts providers exist, keep Reactions
outside default profiles, and retain both enabled and disabled Forum composition
evidence.

Before release, regenerate `Cargo.lock`, execute SQLite and PostgreSQL migrations,
retain replay/concurrency/rollback evidence, add semantic reaction events and
provide reconciliation for catalog and aggregate drift.

## Verification

```bash
cargo test -p rustok-reactions-api
cargo test -p rustok-reactions
cargo test -p rustok-forum reaction_subject
cargo check -p rustok-reactions-api --all-targets
cargo check -p rustok-reactions --all-targets
cargo check -p rustok-forum --all-targets
node scripts/verify/verify-reactions-foundation.mjs
node scripts/verify/verify-reactions-owner-persistence.mjs
node scripts/verify/verify-forum-reaction-subject-provider.mjs
git diff --check
```

Tests, checks, lockfile generation and runtime evidence are maintainer-run.
