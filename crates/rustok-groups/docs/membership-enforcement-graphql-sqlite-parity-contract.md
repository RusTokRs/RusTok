# Groups membership-enforcement native/GraphQL SQLite parity contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_membership_enforcement_graphql_sqlite_parity.rs` retains executable source evidence that the direct Groups membership-enforcement owner port and the stable final Groups GraphQL mutation root expose the same suspend/revoke, replay and CAS semantics on SQLite.

The packet uses one real temporary SQLite database, every production Groups migration, `GroupMembershipEnforcementCommandService`, and `graphql_application_cas::{GroupsQueryRoot, GroupsMutationRoot}`. It adds no alternate enforcement owner, private GraphQL schema, moderation persistence dependency, fallback, manifest or lockfile change.

## Equivalent owner fixtures

Two equivalent Groups contain the same active local owner and ordinary active target member. One group is exercised through native `GroupMembershipEnforcementCommandPort`; the other through the final GraphQL mutations.

GraphQL receives the authenticated tenant-bound owner with an empty effective permission list. The test therefore proves local owner authority through the Groups owner domain instead of a transport-side `groups:moderate` shortcut.

## Suspend and replay parity

Both surfaces suspend their equivalent target at membership revision one with the same reason semantics. The evidence compares:

- target and membership identity relative to each equivalent group;
- membership revision;
- group version;
- lifecycle member count;
- effective status;
- enforcement revision;
- expiry/revocation presence;
- replay flag.

The exact suspension request is then repeated with the same idempotency key while the target is already suspended. Native and GraphQL must return the stored suspension result with `replayed=true`, not fail current-state `already_suspended` validation.

## Fresh stale-CAS parity

A separate fresh idempotency key attempts another suspension using stale expected membership revision one after the committed suspension advanced the revision to two.

Native must return non-retryable `PortErrorKind::Conflict` with stable owner code `groups.membership_enforcement_revision_conflict`. GraphQL must preserve the same owner-safe message and expose:

- transport `code=BAD_USER_INPUT`;
- `domainCode=groups.membership_enforcement_revision_conflict`;
- `retryable=false`.

The stale attempt must not mutate owner state.

## Revoke and historical replay parity

Both surfaces revoke the active direct-local suspension at expected membership revision two. Matching results must advance the target membership revision to three and group version exactly once while lifecycle `member_count` remains unchanged. The enforcement row remains as bounded history with a revocation timestamp and updated enforcement revision.

The exact revoke request is replayed with the same key after the suspension is no longer active; both surfaces must return the stored revoke result with `replayed=true`.

Finally, the original suspend key/request is replayed **after the later revoke commit**. Both native and GraphQL must still return the historical suspended result, including its earlier group version and `revoked_at=None`. This retains evidence that completed receipt replay precedes current authorization/CAS/lifecycle validation and is not rewritten from current projection state.

## Final owner state

Direct SQLite reads are used only after production owner mutations to retain evidence. Both equivalent groups must end with:

- stored target role `member` and lifecycle status `active`;
- target membership revision three;
- lifecycle member count two;
- direct-local enforcement source;
- matching enforcement revision;
- non-null revocation marker;
- final group version equal to the successful revoke result.

## Final-root composition

The schema uses the stable module `graphql_application_cas::GroupsQueryRoot` and `GroupsMutationRoot`; it never instantiates `GroupsMembershipEnforcementMutation` as a private alternate schema.

## Execution status

The packet was not executed while preparing this slice. FBA `membership_enforcement_command_transport_parity` remains null until maintainer execution.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_membership_enforcement_graphql_sqlite_parity -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-membership-enforcement-graphql-sqlite-parity.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, browser/schema execution, workflow, or CI job was run while adding this source.
