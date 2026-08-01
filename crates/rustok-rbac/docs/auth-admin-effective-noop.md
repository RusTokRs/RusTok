# Auth admin effective RBAC mutation contract

## Status

`source_ready_unvalidated`

This correction continues `cycle-001` at `core/rbac`. It does not advance or complete the component.

## Finding

The Auth admin `update_user` orchestration previously treated the presence of a role or status field as an authorization change:

```text
requested_role.is_some() || requested_status.is_some()
```

An exact replay such as assigning the user's existing single canonical role, or sending the user's current status, therefore reserved a new durable RBAC invalidation generation and triggered post-commit cache fan-out. Replaying a non-active status could also revoke sessions again even though the status did not change.

This contradicted the existing committed-role contract, where an exact single-role replacement is a generation no-op.

## Correction

- `RbacService::replace_user_role_in_transaction_if_changed` reports whether a caller-owned transaction actually repaired or replaced the role relation.
- Exactly one assignment to the requested built-in role returns `false` without rewriting relations.
- A matching role among multiple or malformed assignments is still repaired and returns `true`.
- Auth admin compares requested status with the locked user row rather than the pre-lock snapshot.
- Status is written only when it changes.
- A request containing only exact role/status replay does not issue a user-row update.
- Session revocation for a disabled status occurs only when the status transitions to a non-active value.
- Durable generation reservation and post-commit fan-out occur only when the role relation or status effectively changes.
- Permission, hierarchy, tenant, target-management and last-active-super-admin checks remain mandatory even for replayed input.

## Ownership boundary

Auth owns user mutation orchestration and session lifecycle. RBAC remains the owner of relation shape and exact-role change detection. The transaction helper does not commit, reserve a generation, clear caches, publish invalidations or weaken continuity checks; those remain the caller's responsibilities.

## Regression coverage

- transaction-owned exact role replay reports `changed=false`;
- a matching role among multiple assignments reports `changed=true` and repairs the relation;
- status effective-change classification ignores exact replay;
- the user row update is conditional and absent for role/status-only replay;
- `rbac_auth_admin_effective_noop_guard.rs` prevents restoration of presence-based generation reservation, unconditional user-row update, or the old unconditional transaction role write.

## Validation boundary

No formatting, compilation, Rust tests, PostgreSQL, Redis, workflows or CI execution is claimed for this source packet.

Targeted commands:

```bash
cargo test -p rustok-server transaction_role_replacement_reports_exact_noop
cargo test -p rustok-server transaction_role_replacement_repairs_multiple_assignments
cargo test -p rustok-server status_effective_change_ignores_exact_replay
cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard
cargo test -p rustok-server --test rbac_mutation_api_architecture_guard
cargo check -p rustok-server --lib
cargo clippy -p rustok-server --lib -- -D warnings
```
