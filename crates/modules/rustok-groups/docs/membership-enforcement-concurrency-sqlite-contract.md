# Groups direct membership-enforcement SQLite concurrency contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_membership_enforcement_concurrency_sqlite.rs` retains focused concurrency source evidence for the direct `GroupMembershipEnforcementCommandPort` itself. It complements governance-vs-enforcement stress by testing duplicate receipt convergence and same-target expected-revision contention entirely inside the enforcement command boundary.

The packet uses a real temporary SQLite file, every production Groups migration, single-connection command pools, WAL mode and `PRAGMA busy_timeout=5000`. All command connections are opened before the barrier is released. Storage lock errors, task panic, timeout, both-success in distinct-key cases, or other owner errors are not accepted outcomes.

## Same-key concurrent suspension

Two independent production command services receive the exact same actor, request and idempotency key at target membership revision one and start together.

The group serialization/receipt contract must converge to:

- both calls succeed;
- exactly one result has `replayed=false` and exactly one has `replayed=true`;
- both expose the same material owner result;
- membership revision is two;
- group version is two;
- enforcement revision is one;
- lifecycle member count remains two;
- exactly one active enforcement row exists;
- exactly one audit fact, one membership semantic event and one command receipt exist.

This proves concurrent duplicate delivery is collapsed by the owner receipt boundary rather than producing a unique-key/storage error or a second mutation.

## Distinct-key concurrent suspension

Two independent requests use different idempotency keys but the same prepared expected membership revision one.

Exactly one suspension may commit. The loser must fail non-retryably with `groups.membership_enforcement_revision_conflict`, not `already_suspended` or a storage error. Final state must remain group version two, target membership revision two, enforcement revision one, active enforcement, unchanged member count two, and exactly one audit/event/receipt triplet.

This retains the ordering contract that expected membership revision is evaluated after serialization and before current enforcement-state validation.

## Distinct-key concurrent revoke

A fresh group first receives one baseline direct suspension, reaching membership revision two and group version two. Two different revoke keys then race at expected revision two.

Exactly one revoke may commit. The loser must fail non-retryably with `groups.membership_enforcement_revision_conflict`. Final state must be:

- membership revision three;
- group version three;
- enforcement revision two;
- revoked enforcement row;
- lifecycle member count two;
- exactly two audit/event/receipt triplets total: baseline suspend plus the winning revoke.

## SQLite serialization

The packet does not accept `database is locked` or `groups.persistence_unavailable` as business outcomes. File-backed WAL plus per-connection busy timeout is used so the production SQLite writer reservation must serialize the same-group command pair.

Every pair is bounded by a 30-second timeout; a stalled writer reservation or deadlock is evidence failure.

## Execution status

The packet was not executed while preparing this slice. FBA `membership_enforcement_command_concurrency` remains null until maintainer execution.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_membership_enforcement_concurrency_sqlite -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-membership-enforcement-concurrency-sqlite.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, browser/schema execution, or CI job was run while adding this source.
