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
| `REACTIONS-00` | `in_progress` | Neutral API, optional module registration and provider registry exist; maintainer verification remains. |
| `REACTIONS-01` | `planned` | PostgreSQL/SQLite owner persistence, catalog revisions, actor uniqueness and idempotent command receipts. |
| `REACTIONS-02` | `planned` | Atomic aggregate updates, semantic events and reconciliation. |
| `REACTIONS-03` | `planned` | Forum topic/reply subject adapter and degraded profile. |
| `REACTIONS-04` | `planned` | Second real producer adapter and neutral-contract review. |
| `REACTIONS-05` | `planned` | Bounded read/write transports and module-owned UI. |
| `REACTIONS-06` | `planned` | Runtime evidence, FBA contracts, import/reconciliation and release profiles. |

## Ownership

Reactions owns catalog revisions, actor reaction state, write receipts and
aggregate projections. Producer modules own subject existence, content,
revision, visibility and reaction policy. Profiles owns actor presentation.
Reputation and achievements consume semantic facts but do not mutate reaction
state. Notifications may consume reaction events but are not part of reaction
command correctness.

## Immediate next action

Implement `REACTIONS-01` as one bounded persistence owner. Require tenant-
composite identity, one actor/key relation, one command UUID payload identity,
explicit catalog revision, checked single/multi selection and atomic aggregate
changes. Do not add Forum adapters, transports or UI in the persistence PR.

## Verification

```bash
cargo test -p rustok-reactions-api
cargo test -p rustok-reactions
cargo check -p rustok-reactions-api --all-targets
cargo check -p rustok-reactions --all-targets
node scripts/verify/verify-reactions-foundation.mjs
git diff --check
```

Tests and checks are maintainer-run.
