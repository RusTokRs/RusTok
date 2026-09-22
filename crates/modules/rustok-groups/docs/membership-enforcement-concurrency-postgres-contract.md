# Groups direct membership-enforcement PostgreSQL concurrency contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_membership_enforcement_concurrency_postgres.rs` is the PostgreSQL counterpart to the SQLite focused direct-command concurrency packet. It retains evidence for concurrent receipt convergence and same-target expected-revision fencing entirely inside `GroupMembershipEnforcementCommandPort`.

The packet adds no alternate owner implementation, Moderation adapter, dependency, manifest, lockfile, registry promotion, GraphQL path, or production behavior change.

## PostgreSQL isolation

The ignored packet creates one unique PostgreSQL schema. Every command pool is single-connection and receives that schema through startup options:

```text
options=-csearch_path=<schema>,public
```

No session-local `SET search_path` is used. All competing command connections are opened before the barrier is released. A PostgreSQL deadlock/storage error, task panic, timeout, or unexpected owner error is evidence failure rather than a valid business outcome.

## Same-key concurrent suspension

Two independent services receive the exact same actor, request and idempotency key at target membership revision one.

Required outcome:

- both calls succeed;
- exactly one is the material commit and exactly one reports `replayed=true`;
- both expose the same owner result;
- target membership revision becomes two;
- group version becomes two;
- enforcement revision is one and active;
- lifecycle member count remains two;
- exactly one audit/event/receipt triplet exists.

Concurrent duplicate delivery must therefore converge through the stored receipt rather than produce a unique-key failure or second mutation.

## Distinct-key concurrent suspension

Two different idempotency keys race with expected membership revision one. Exactly one suspension commits. The loser must fail non-retryably with `groups.membership_enforcement_revision_conflict`.

Final state remains group version two, target membership revision two, enforcement revision one, active enforcement, member count two, and one audit/event/receipt triplet. `already_suspended` is not the accepted loser outcome because expected-revision fencing is evaluated after group serialization and before current enforcement-state validation.

## Distinct-key concurrent revoke

A fresh group receives one baseline suspension first, reaching membership revision two and group version two. Two different revoke keys then race at expected revision two.

Exactly one revoke commits. The loser must fail non-retryably with `groups.membership_enforcement_revision_conflict`. Final state must be membership revision three, group version three, enforcement revision two, revoked enforcement, member count two, and two total audit/event/receipt triplets: baseline suspension plus winning revoke.

## Timeout/deadlock boundary

Every pair is bounded by 30 seconds. The packet accepts no PostgreSQL deadlock error, timeout, retryable persistence failure, both-success distinct-key outcome, or both-fail outcome.

## Execution status

The test is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and was not executed while preparing this slice. FBA `membership_enforcement_command_concurrency` remains null until maintainer execution.

Maintainer command:

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_membership_enforcement_concurrency_postgres -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-membership-enforcement-concurrency-postgres.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, browser/schema execution, or CI job was run while adding this source.
