# Groups governance native/GraphQL SQLite parity contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_governance_graphql_sqlite_parity.rs` retains executable source evidence that the native `GroupGovernanceCommandPort` and the stable final Groups GraphQL root expose the same owner-domain governance semantics on SQLite.

The packet uses a real temporary file-backed SQLite database, every production Groups migration, `GroupGovernanceService`, and `graphql_application_cas::{GroupsQueryRoot, GroupsMutationRoot}`. It adds no alternate governance owner, private GraphQL schema, fallback, dependency, manifest, lockfile, or production behavior change.

## Equivalent owner fixtures

Two equivalent Groups start with the same aggregate version and the same local principals:

- current owner;
- active administrator;
- ordinary role-change target;
- ordinary ownership-replacement target.

One group is mutated through native governance ports and the other through the stable final GraphQL mutation root. GraphQL receives authenticated tenant-bound users with an empty effective permission list, so local governance remains a Groups owner-domain decision rather than a transport-side platform permission shortcut.

## Parity contract

The packet retains parity for both governance commands.

### Role change

An active administrator changes an ordinary member from `member` to `moderator` through native `change_group_role` and GraphQL `changeGroupRole`. The evidence requires matching previous/current roles, actor/target identity, group version and `replayed=false`.

The exact same native and GraphQL requests are then repeated with the same idempotency keys. Both surfaces must return the stored result with `replayed=true` and no extra aggregate version advance.

### Ownership transfer

Before the successful transfer, the administrator attempts to transfer ownership. Native must return non-retryable `groups.forbidden` with `PortErrorKind::Forbidden`; GraphQL must return the same owner-safe message classified as `PERMISSION_DENIED`. The denied request must leave both equivalent aggregates at the post-role-change version and original owner.

The current owner then transfers ownership to the ordinary replacement member through native `transfer_group_ownership` and GraphQL `transferGroupOwnership`. Both surfaces must return matching owner result semantics and the same next aggregate version.

The exact transfer is replayed after the owner reference has already moved. Both native and GraphQL must still return the stored result with `replayed=true`, proving transport parity for the governance receipt-first contract rather than re-authorizing the former owner against current state.

## Final owner state

The packet reads final owner rows only as evidence after all mutations have gone through production owner surfaces. Both equivalent groups must have:

- `groups.owner_user_id` set to the replacement;
- former owner role `admin`;
- replacement role `owner`;
- role-change target `moderator`;
- original administrator still `admin`;
- the final group version equal to the ownership-transfer result.

No direct table write is used to stand in for governance behavior.

## Final-root composition

The schema is built with the module's stable `graphql_application_cas::GroupsQueryRoot` and `GroupsMutationRoot`. The evidence does not instantiate `GroupsGovernanceMutation` as a private alternate schema.

## Execution status

The packet was not executed while preparing this slice. FBA `governance_transport_parity` therefore remains null until maintainer execution.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_governance_graphql_sqlite_parity -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-governance-graphql-sqlite-parity.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, browser/schema execution, workflow, or CI job was run while adding this source.
