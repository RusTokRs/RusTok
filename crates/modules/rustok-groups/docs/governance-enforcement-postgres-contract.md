# Groups governance/enforcement PostgreSQL evidence contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_governance_enforcement_postgres.rs` retains executable PostgreSQL evidence for the GROUPS-07 owner-lock and replay boundary shared by governance and membership enforcement.

The test is compiled only with the server `mod-groups` feature and is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured. It uses the production `rustok-groups` services and production Groups migrations. No duplicate governance or enforcement implementation exists in the fixture.

## Isolation

Every execution creates a unique PostgreSQL schema and connects through a schema-scoped PostgreSQL startup `search_path`. This matters because SeaORM uses a connection pool: a one-shot `SET search_path` on one pooled session would not be a safe isolation mechanism for subsequent queries on other sessions.

The fixture applies all production migrations returned by `rustok_groups::migrations::migrations()` inside the isolated schema, opens independent pooled connections for governance/enforcement calls, and drops the schema after the evidence assertions.

## Evidence 1: receipt-first lost-response replay and actor binding

The fixture starts with an owner, an admin and active members.

1. The admin changes a member from `member` to `moderator` through `GroupGovernanceCommandPort`.
2. The owner then suspends that admin through `GroupMembershipEnforcementCommandPort`.
3. The now-suspended admin retries the exact original governance command with the exact original idempotency key.
4. The stored result must replay successfully with `replayed=true`, the same previous/current role and the original group version.
5. A different actor using that idempotency key and the same request must conflict.
6. A fresh governance command from the suspended admin must fail with `groups.membership_suspended`.

This proves the intended ordering: group serialization -> actor-bound completed receipt -> current effective authorization.

## Evidence 2: concurrent role mutation versus direct suspension

Two independent PostgreSQL connections race against the same active member and the same prepared membership revision:

- governance attempts `member -> moderator`;
- direct enforcement attempts `SuspendGroupMembershipRequest`.

Both production commands serialize on the Groups owner row before their membership/enforcement work. Exactly two outcomes are accepted:

- **role wins first**: role becomes `moderator`; the later suspension sees a stale reviewed membership revision and returns `groups.membership_enforcement_revision_conflict`; no enforcement row commits;
- **suspension wins first**: the enforcement row commits; the later governance command resolves the member as suspended and returns `groups.membership_suspended`; stored role remains `member`.

The test rejects any outcome in which both commands succeed. It also requires the raced membership revision to advance by exactly one material change.

This is the executable concurrency counterpart to the deterministic lock order `Group -> membership rows in user UUID order -> enforcement rows in membership-ID order`.

## Evidence 3: platform ownership recovery

The direct local enforcement command correctly forbids suspending the current group owner, so the fixture installs the already-defined moderation-owned enforcement projection shape directly for the current owner. This does **not** stand in for the missing neutral Moderation adapter; it only creates the owner state that governance recovery must safely consume.

The fixture records:

- `state=suspended`;
- `source_kind=moderation_decision`;
- bounded moderation decision UUID/hash provenance;
- stored restore status `active`;
- a service actor;
- the membership revision trigger effect and a matching group-version advance.

A platform actor with `groups:manage` then transfers ownership to an effective-active replacement member through the real `GroupGovernanceCommandPort`.

The evidence requires:

- the suspended current-owner enforcement row to be resolved through the normal owner-clock resolver;
- platform recovery to succeed only for the explicitly recoverable current-owner state;
- the replacement owner to remain effective-active;
- old owner role to become `admin`;
- replacement role to become `owner`;
- `groups.owner_user_id` to point at the replacement.

Corrupt or unsupported current-owner enforcement still fails closed in production governance source.

## What this source does not prove until executed

The file is intentionally ignored and was not executed while preparing this slice. Therefore the following remain open evidence gates:

- actual PostgreSQL scheduler/timing behavior on the maintainer environment;
- transport parity;
- SQLite parity;
- repeated stress/deadlock statistics;
- Moderation adapter application/lost-response behavior;
- CI/runtime readiness promotion.

## Maintainer execution

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_governance_enforcement_postgres -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-governance-enforcement-postgres.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this evidence source.
