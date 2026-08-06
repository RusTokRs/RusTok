# M6 drift finding lifecycle commands

Status: `source_complete_repair_pending`.

## Purpose

This slice adds one internal fail-closed command boundary for closing an exact open Index drift
finding as either `resolved` or `ignored`.

It does not discover findings, confirm candidates, record mismatch evidence, expose a public command,
or authorize repair. The existing `index_consistency_findings` row remains the owner of current
finding state.

## Command contract

`IndexDriftFindingLifecycleCommand` requires:

- one non-nil tenant UUID;
- one non-nil finding UUID;
- one non-nil command UUID used for idempotent retry identity;
- action `Resolve` or `Ignore`;
- explicit expected state `Open`;
- actor kind bounded to 32 machine-name bytes;
- actor subject bounded to 191 non-control bytes;
- a trimmed nonempty reason bounded to 512 bytes.

The command does not accept a finding key, check name, scope, digest, timestamp, target state,
raw audit JSON, SQL, or repair instruction. The action derives the only supported target state.

Command and actor `Debug` implementations omit actor subject text and reason text. Failures expose
only retryable/permanent classification and a bounded machine code.

## Fail-closed authorization

`IndexDriftFindingLifecycleService` always calls
`IndexDriftFindingLifecycleAuthorizer::authorize` before the store.

A denied authorization returns only `Denied`; the store is not called and finding existence is not
observed.

The PostgreSQL store cannot accept an ordinary command. It implements
`apply_authorized_lifecycle_command` and requires
`IndexDriftFindingAuthorizedLifecycleCommand`, whose constructor is private to the application
service. This prevents direct store use from bypassing the authorization boundary.

No default allow-all authorizer is provided. This slice does not compose an authorizer into server
runtime extensions.

## PostgreSQL transaction

`PostgresIndexDriftFindingLifecycleStore` uses one PostgreSQL
`SERIALIZABLE READ WRITE` transaction for every authorized command.

The transaction:

1. acquires an advisory transaction lock on tenant plus command UUID;
2. reads an existing lifecycle event for the command UUID;
3. returns `AlreadyApplied` only when the complete stored payload matches;
4. rejects reuse of the command UUID for another finding, action, actor, reason, or state;
5. locks the exact tenant/finding row with `FOR UPDATE`;
6. returns `FindingNotFound` or `StateChanged` without writing when the precondition fails;
7. updates only `state` and `closed_at` from open to the action-derived target;
8. inserts exactly one audit event in the same transaction;
9. commits the state transition and audit event atomically.

The finding key, check name, scope, severity, first/last detection timestamps, details, and digest
evidence are not modified by the lifecycle command.

## Idempotency

The primary key of `index_consistency_finding_lifecycle_events` is
`(tenant_id, command_id)`.

A retry with the same exact payload returns the original typed target-state receipt even when the
finding was later reopened by a new detection. Reusing the command UUID with any different payload
fails permanently as `index_drift_finding_lifecycle_command_id_conflict`.

Concurrent different commands for the same finding serialize on the finding row. Only the command
that observes the explicit open precondition may transition it; later commands return the current
state without adding audit rows.

## Audit storage

Migration `m20260806_000006_add_index_finding_lifecycle_audit` adds the Index-owned lifecycle ledger.
Each row stores only:

- tenant, command, and finding UUIDs;
- action, from-state, and to-state;
- bounded actor kind and actor subject;
- bounded reason;
- database-owned creation timestamp.

The table has a composite foreign key to the exact finding and database checks for supported actions,
states, transitions, and bounded text. PostgreSQL and SQLite triggers reject row updates. The Index
lifecycle API contains no update or delete operation for audit rows.

Audit rows follow the existing finding/tenant cascade retention behavior. This permits whole-tenant
retention deletion while preventing lifecycle code from rewriting historical events.

## Outcomes

The service returns only:

- `Denied`;
- `Applied(receipt)`;
- `AlreadyApplied(receipt)`;
- `NotApplied(FindingNotFound)`;
- `NotApplied(StateChanged { current })`;
- bounded retryable/permanent dependency failure.

Receipts contain only command UUID, finding UUID, and resulting state. SQL, database causes, actor
subject, reason, timestamps, and stored row contents are not returned.

## Deliberate limits

This slice does not add:

- GraphQL, HTTP, CLI, MCP, native-admin, or module-runtime composition;
- an allow-all or request-derived authorization implementation;
- audit inspection or public actor/reason disclosure;
- finding candidate iteration or automatic closure;
- targeted, shadow, full, or automatic repair;
- retained migration, PostgreSQL, concurrency, workflow, or CI evidence.

## Next implementation step

Add one internal targeted repair boundary for an exact open confirmed finding. It must require an
authorized operator, capture admitted before evidence, apply only the finding-specific repair owner,
re-read admitted after evidence, and record a separate repair receipt. Repair must not be inferred
from lifecycle state and must remain unmounted from public transports.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_finding_lifecycle -- --nocapture
node scripts/verify/verify-index-drift-finding-lifecycle.mjs
node scripts/verify/verify-index-confirmed-candidate-persistence.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, migrations, PostgreSQL scenarios, workflows, or CI were
executed by the implementation agent.
