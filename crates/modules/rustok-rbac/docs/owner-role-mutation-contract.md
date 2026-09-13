# Owner role mutation contract

Status: `in_progress`

This cycle-001 slice defines the first approved owner-level operator mutation
contract for canonical built-in user roles. It does not add a new role-management
transport and does not complete the wider custom-role or arbitrary-permission
management backlog.

The machine-readable contract is:

```text
crates/modules/rustok-rbac/contracts/rbac-owner-role-mutation-contract.json
```

## Existing transport composition

The existing RBAC GraphQL mutation already delegates through
`ServerRbacGraphqlRoleWriter` into `UserAdminMutationRuntime`. Native user
administration uses the same mutation runtime. This slice keeps that composition
and changes the policy and persistence seam underneath it.

No `/roles` endpoint, parallel GraphQL writer, new native command or direct host
relation write is introduced.

## Owner policy

`rustok-rbac::plan_user_role_mutation` evaluates authoritative facts collected by
the transaction-backed owner operation:

- requested tenant identity;
- authenticated actor identity, actor tenant and request-bound actor role;
- locked target identity, tenant, status and authoritative effective role;
- whether the target has exactly one requested canonical tenant-role assignment;
- the number of remaining active SuperAdmin users after excluding the target when
  continuity is at risk.

The owner validates non-nil identities, actor and target tenant equality, role
assignment hierarchy, target-management hierarchy and last-active-SuperAdmin
continuity. `plan_persisted_user_role_mutation_on` locks the target and reads its
status, effective permissions, exact assignments, and remaining active
administrators inside the host transaction. The authenticated host supplies
request-bound actor authority and the requested change.

The shared `ensure_user_authority_continuity_on` checks status-only changes and
account removal from persisted target authority. Inactive targets do not reduce
the active administrator set. The canonical system-role lock serializes
continuity counts on PostgreSQL; SQLite obtains its write lock through an
unchanged-value update before reading the count. Ordinary committed replacement
uses `replace_persisted_user_role_on`; the host owns commit and cache delivery.

The policy returns one of three semantic results:

1. `Noop` when one exact canonical assignment already equals the requested role;
2. `AssignmentRepaired` when the effective role is unchanged but the relation set
   is malformed or contains multiple assignments;
3. `RoleReplaced` when the effective built-in role changes.

Separating repair from replacement prevents a relation cleanup from being
reported as a privilege transition.

## Transaction and event boundary

For an applying role plan, `ServerAuthAdminMutationProvider::update_user` performs
one transaction:

1. lock and reload the target user;
2. collect owner facts and request a role-mutation plan;
3. apply the requested user fields;
4. replace or repair the role relation through
   `RbacService::replace_user_role_in_transaction`;
5. reserve the durable RBAC invalidation generation;
6. build the typed event from the owner plan using that exact generation;
7. insert the registered typed event into the canonical outbox through
   `TransactionalEventBus::publish_contract_in_tx`;
8. commit;
9. invalidate the local snapshot and attempt fast local/Redis fan-out.

Typed event insertion failure explicitly rolls back the user row, relation
mutation and generation reservation. Post-commit fan-out remains best-effort;
the committed database generation remains the recovery authority.

## Typed events

The sealed `rustok-events` family `RbacRoleMutationEvent` registers two version-1
contracts:

```text
rbac.user_role_replaced
rbac.user_role_assignment_repaired
```

`rbac.user_role_replaced` carries the target user, previous role, new role and
committed durable generation. The roles must differ.

`rbac.user_role_assignment_repaired` carries the target user, retained role and
committed durable generation. Both events accept only the four canonical built-in
role slugs and reject nil users or generation zero.

Durable publication uses the registered sealed `rustok-events` contracts.
The unused parallel source event payload, its constructors, exports, and tests
were deleted during the ownership cutover. There is one canonical event family
per mutation capability.

`require_request_role_grant` owns the privilege-delegation ceiling: privileged
roles must fit within the token's effective permissions, while Customer remains
the baseline account role. The server reads the immutable request snapshot and
delegates this rule to the owner. `require_role_assignment` and
`require_user_management` share the same hierarchy policy with the role planner.

`authorize_current_permission` uses the canonical persisted relation reader and
tenant policy engine on the caller's connection or transaction. Transaction-local
grant revocation must deny a current decision before commit; rollback restores
the persisted authority. This uncached read does not constitute a revocation fence.

## Deliberate boundary

This slice covers canonical built-in role replacement and malformed-assignment
repair through the existing user-admin facade. It does not define:

- custom role creation, deletion or hierarchy;
- arbitrary permission assignment to custom roles;
- a remote/headless role-administration product contract;
- a new REST, GraphQL or native transport;
- status-only or account-deletion event redesign;
- executed outbox atomicity, transport or parity evidence.

The broader P1 item remains open until permission mutation ownership, native
operator parity and the remote/headless product decision are complete.

## Verification boundary

The initial source report did not run Rust tests or execution gates. The
2026-09-13 ownership cutover adds a SQLite runtime regression for owner-read
facts, tenant isolation, rejected last-active-administrator demotion and
deactivation, exact no-op, malformed-assignment repair, and inactive-target
removal. The focused owner tests pass 6/6. Server runtime, PostgreSQL concurrency,
outbox failure, transport parity, and wider permission-management evidence must
be assessed separately. The platform cursor remains on `core/rbac`; bounded
ownership tests do not promote the broader readiness status. The complete
RBAC lib tests pass 73/73 and scoped clippy passes; server committed-role tests
pass 4/4. These results do not prove the fresh purge-policy transaction host
test, PostgreSQL concurrency, or transport/Outbox parity.

Targeted maintainer commands:

```bash
cargo test -p rustok-events rbac_role_mutation
cargo test -p rustok-rbac role_mutation
cargo test -p rustok-server auth_admin_mutation_provider
node scripts/verify/verify-rbac-owner-role-mutation-contract.mjs
```
