---
id: doc://crates/rustok-reactions-api/docs/implementation-plan.md
kind: capability_implementation_plan
language: en
status: active
owners:
  - rustok-reactions
last_reviewed: 2026-08-06
---

# `rustok-reactions-api` implementation plan

## Current status

`REACTIONS-00` is `in_progress`. The neutral source contract provides bounded
semantic keys, revisioned subjects, explicit catalog selection policy,
idempotency identity, actor/aggregate snapshots, typed read/write ports and a
unique source-provider/factory registry.

No runtime validation is claimed yet.

## Remaining foundation scope

1. Maintainer compilation and unit verification.
2. Add the `rustok-reactions` persistence owner with tenant-composite integrity,
   immutable catalog revisions, actor uniqueness and command receipts.
3. Add transactional semantic events and bounded aggregate reconciliation.
4. Add the first real source adapter in Forum for topic/reply authorization.
5. Add a second producer before freezing cross-module presentation contracts.
6. Add GraphQL/native transports and module-owned UI only after owner commands
   and degraded profiles are executable.

## Invariants

- The neutral API never imports a producer domain crate.
- Subject providers never grant access using copied transport policy.
- A stale subject revision conflicts or becomes unavailable; it is not silently
  rewritten to a newer target.
- Command UUID reuse with different actor/subject/reaction/action must conflict
  in the future persistence owner.
- Forum votes remain outside this contract until an explicit migration exists.
- Reputation, achievements and notifications are separate capabilities.

## Verification

```bash
cargo test -p rustok-reactions-api
cargo check -p rustok-reactions-api --all-targets
node scripts/verify/verify-reactions-foundation.mjs
git diff --check
```

Tests and checks are maintainer-run.
