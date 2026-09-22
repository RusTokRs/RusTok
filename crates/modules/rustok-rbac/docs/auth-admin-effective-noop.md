# Auth admin effective no-op contract

## Status

`source_ready_unvalidated`

Merged PR #2867 owns canonical built-in role policy and durable role mutation events.
This correction narrows the host adapter to effective status and user-row changes.

## Exact replay

After the tenant-scoped user row is locked:

- a requested status equal to the locked status is not a status change;
- an exact canonical role request is classified by `plan_user_role_mutation` as `Noop`;
- a request containing only exact role/status replay does not update `users`;
- it does not reserve a durable authorization generation;
- it does not revoke sessions;
- it does not publish a role event or invalidation fast path.

Email, name, password, metadata, real status transitions, role replacement, and malformed
role-relation repair remain real writes according to their owned policies.

## Relation repair

A matching effective role among multiple or malformed tenant assignments is not an
exact replay. The RBAC owner returns `RbacRoleMutationOutcome::Apply` with
`AssignmentRepaired`; the server repairs the relation, reserves a generation, and
publishes the merged `rbac.user_role_assignment_repaired` contract in the same
transaction.

## Status transitions

A real transition to `inactive` or `banned` revokes active sessions. Replaying an
already inactive or banned status does not revoke them again. Status-only transitions
advance the durable authorization generation but do not publish a role-mutation event.

## Required verification

```bash
cargo check -p rustok-server --lib
cargo test -p rustok-server status_effective_change_ignores_exact_replay
cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard
cargo test -p rustok-server auth_admin_mutation_provider
```

No formatting, compile, test, database, Outbox, or runtime execution is claimed by this
source-only correction.
