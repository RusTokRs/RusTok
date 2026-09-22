# Groups governance native/GraphQL PostgreSQL parity contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_governance_graphql_postgres_parity.rs` is the PostgreSQL counterpart to the SQLite governance transport packet. It retains executable source evidence that native `GroupGovernanceCommandPort` and the stable final Groups GraphQL root expose the same Groups-owned governance semantics.

The packet uses a unique PostgreSQL schema, every production Groups migration, `GroupGovernanceService`, and `graphql_application_cas::{GroupsQueryRoot, GroupsMutationRoot}`. It adds no alternate governance owner, private GraphQL schema, fallback, dependency, manifest, lockfile, or production behavior change.

## PostgreSQL isolation

Every pooled connection receives the isolated schema through PostgreSQL startup options:

```text
options=-csearch_path=<schema>,public
```

The packet intentionally does not rely on session-local `SET search_path`.

## Equivalent owner fixtures

Two equivalent Groups start with the same aggregate version and the same local principals: current owner, active administrator, ordinary role-change target and ordinary ownership-replacement target. One group is mutated through native governance ports and the other through the stable final GraphQL mutation root.

GraphQL receives authenticated tenant-bound users with an empty effective permission list. Local owner/admin authority therefore remains a Groups owner-domain decision rather than a transport-side platform permission shortcut.

## Parity contract

### Role change

An active administrator changes an ordinary member from `member` to `moderator` through native `change_group_role` and GraphQL `changeGroupRole`. Both surfaces must expose matching previous/current roles, actor/target identity, group version and `replayed=false`.

The exact native and GraphQL requests are repeated with the same idempotency keys. Both surfaces must return the stored result with `replayed=true` and no extra aggregate version advance.

### Ownership transfer

The administrator first attempts to transfer ownership. Native must return non-retryable `groups.forbidden` with `PortErrorKind::Forbidden`; GraphQL must return the same owner-safe message classified as `PERMISSION_DENIED`. The denied request must leave both aggregates at the post-role-change version and original owner.

The current owner then transfers ownership to the ordinary replacement member through native `transfer_group_ownership` and GraphQL `transferGroupOwnership`. Both surfaces must return matching result semantics and the same next aggregate version.

The exact transfer is replayed after `groups.owner_user_id` has already moved. Both native and GraphQL must return the stored result with `replayed=true`, retaining the governance receipt-first contract instead of re-authorizing the former owner against current state.

## Final owner state

Direct PostgreSQL reads are used only after production owner mutations to retain evidence. Both equivalent groups must have replacement `owner_user_id`, former owner role `admin`, replacement role `owner`, role-change target `moderator`, original administrator `admin`, and the exact ownership-transfer group version.

## Final-root composition

The schema uses the stable module `graphql_application_cas::GroupsQueryRoot` and `GroupsMutationRoot`. It never instantiates `GroupsGovernanceMutation` as an alternate schema.

## Execution status

The test is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and was not executed while preparing this slice. FBA `governance_transport_parity` remains null until maintainer execution.

Maintainer command:

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_governance_graphql_postgres_parity -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-governance-graphql-postgres-parity.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, browser/schema execution, workflow, or CI job was run while adding this source.
