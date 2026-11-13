# Durable operations do not retry unresolved side effects

- Date: 2026-11-12
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform, rustok-installer, rustok-installer-persistence, rustok-modules, rustok-sandbox
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

Two durable interactive paths can outlive the request or process that started
them:

At decision time, the two paths had these recovery gaps:

- The HTTP installer stored a durable `running` job but executed its plan in an
  in-process `tokio::spawn`. The executor created a new install session, stored
  only a redacted plan snapshot, and reported its session ID to the HTTP job
  only after successful completion. There was no resumable executor proving
  which steps and external effects had completed.
- Artifact binding idempotency stored a pending receipt with a 60-second lease.
  An execution error could be returned after guest code performed effects; the
  host then attempted to delete the pending receipt. The host idempotency key
  was not propagated into `ArtifactBindingExecutionContext` or `SandboxContext`.

A time-based lease is an ownership hint, not proof that a previous executor or
external side effect has stopped. Replaying a command because its lease expired,
or deleting its receipt because an attempt returned an error, can duplicate
non-transactional effects. Exactly-once execution across arbitrary external
systems cannot be promised without cooperation from each effect owner.

## Decision

Every durable operation that may cause side effects has one stable operation
identity, a semantic request digest, and a monotonically fenced attempt
identity. Lease expiry alone MUST NOT authorize replay. If the previous attempt
may have crossed a side-effect boundary and its outcome cannot be established,
the operation enters `reconciliation_required` (or the owning equivalent) and
fails closed for ordinary retries.

Automatic retry is allowed only when the owning boundary proves both:

1. the prior attempt is fenced from making further effects; and
2. every already-issued side effect is either transactionally coupled to the
   operation receipt or idempotently recognized by the same stable operation
   identity.

Explicit operator recovery may resume or reissue an operation only after the
owner records the reconciliation evidence, actor, decision, and new attempt
fence. A generic status reset or lease extension is not recovery.

### Installer jobs

The HTTP adapter becomes an admission/status adapter over an owner-managed
persistent job. The durable job records a stable job identity and a safe target
locator before execution. It links that identity to the target install session
before the first irreversible step. A worker claims and renews a fenced lease;
restart recovery loads the same session, plan version, and ordered step receipts
rather than creating a new session.

Durable plans MUST NOT contain plaintext credentials. Recoverable asynchronous
plans may persist only resolvable `SecretValue::Reference` values or owner-issued
secret handles; otherwise admission must reject the plan or require an
operator-supplied secret at recovery. The executor resumes only at a step whose
receipt and effect owner prove the prior step's outcome. An ambiguous
non-transactional step is recorded as reconciliation-required, never replayed
blindly. Control-plane job status and target-database session state are
reconciled through the stable job identity; neither database is assumed to
participate in the other's transaction.

### Artifact binding execution

The stable binding idempotency key is propagated as host-owned execution
context through the versioned sandbox request and capability-call boundary. The
artifact cannot replace or forge it. Every effectful capability that can run
under an idempotency-`Required` binding must either use a stable derived effect
identity and retain its owner receipt, or expose an explicit reconciliation
operation. Required-idempotency admission must fail closed for an effect path
that cannot meet that contract.

A sandbox timeout, executor disconnect, process loss, or lost terminal response
is an ambiguous outcome unless the capability owners provide proof otherwise.
Such an outcome keeps its durable receipt in reconciliation-required state;
ordinary same-key retries report that state and do not dispatch guest code
again. A late completion may settle the receipt only while its attempt fence is
still current. Operator reconciliation can then replay the recorded result,
confirm no effect and issue a new fenced attempt, or record an externally
confirmed terminal outcome. It must never silently erase the original evidence.

## Sources of truth and ownership

- `rustok-installer-persistence` owns durable HTTP job identity and status.
- The installer target database owns the install session, state machine, and
  step/effect receipts. The target session records the stable job identity so
  control-plane and target state can be reconciled without pretending to have a
  distributed transaction.
- `rustok-modules` owns artifact binding operation identity, admission, attempt
  fencing, and reconciliation state.
- `rustok-sandbox` transports the host-issued invocation identity but does not
  decide whether a side effect is replayable.
- Each capability or external-effect owner remains authoritative for its own
  effect receipt and idempotency guarantee. HTTP/GraphQL DTOs are projections,
  not recovery authorities.

## Invariants

### Allowed states

An operation may be admitted, running under one current attempt fence,
completed with a durable result, or reconciliation-required with explicit
uncertainty evidence. A retry may replay a completed result. A retry may claim
an expired attempt only after the previous attempt is fenced and the owner has
reconciled its effects.

### Forbidden states

- An expired timestamp alone must not turn an unresolved operation into a new
  execution.
- An ambiguous failure must not delete a durable idempotency receipt.
- A retry must not create a second installer session for the same job.
- Plaintext database/admin credentials must not be persisted to make a job
  resumable.
- A guest-supplied field must not override the host-issued stable operation key
  or attempt fence.
- The system must not claim global exactly-once effects for external services
  that do not implement idempotency or reconciliation.

## Non-goals

This decision does not make arbitrary third-party APIs transactional, provide
exactly-once delivery across independent databases, or authorize automatic
rollback of already committed installation steps. It does not make every
non-idempotent artifact binding retryable; bindings without safe effect
contracts remain fail-closed after ambiguity.

## Data, transaction, and concurrency boundary

Each owner stores the stable operation key, semantic request digest, current
attempt fence, state, and reconciliation evidence in its canonical persistence.
Unique constraints scope installer jobs and artifact operation receipts to
their actual owner identities. A claim, attempt-generation advance, and receipt
transition use compare-and-set under the owner transaction. Capability effects
persist their own idempotency receipt in the same transaction as their mutation
where possible. Where an external system cannot join that transaction, the
owner uses its stable idempotency key and an explicit reconciliation protocol.
Cross-database installer progress uses a saga and stable job identity, not a
fictional atomic commit.

## Context dimensions

Tenant and actor participate in artifact-binding operation scope and
authorization; installation and binding identities are semantic scope. Installer
jobs are host/control-plane scoped until the target session is bound; the target
session and tenant are authoritative after creation. The stable operation key
and attempt fence participate in effect identity. Trace IDs are observability
evidence only and do not alter replay identity. Secrets are referenced, never
copied into durable receipts.

## Events and projections

Owner state and owner receipts are authoritative. Any operator reconciliation
or terminal transition that affects projections emits its audit/domain event
through the same owner transaction where the storage boundary permits it; a
relay or status endpoint is a projection and cannot advance the recovery state.

## Failure semantics

Validation and authorization failures before execution may reject without a
durable execution claim. Once an operation might have caused effects, errors
are classified as ambiguous unless the effect owner proves they are pre-effect.
Ambiguous outcomes preserve their evidence, surface a stable retryable or
operator-action-required transport code, and fail closed against redispatch.
Storage failures while recording reconciliation are logged and returned without
masking the original error; they do not authorize retry.

## Migration and cutover

Existing installer `running` jobs and artifact `pending` receipts cannot be
assumed safe to replay. Migration MUST preserve them and classify them for
operator reconciliation; it must not blanket-reset or delete them. New schemas
and APIs are introduced by additive migrations and explicit versioned transport
changes. Existing callers are migrated to the new owner contracts before old
paths are removed. No plaintext recovery-plan backfill is permitted.

## Alternatives considered

- **Reset all stale `running`/`pending` rows at startup:** rejected because it
  can replay effects that committed before process loss.
- **Increase the lease duration:** rejected because no finite duration proves
  that an external effect stopped or that the caller observed its result.
- **Keep the current lease and claim exactly-once behavior:** rejected because
  a lease cannot fence an already-running external effect, and the installer
  has no resumable state-machine entry point.
- **Persist raw installer plans:** rejected because they can contain plaintext
  credentials and would turn recovery into a secret-retention path.

## Verification

Required evidence before implementation can be marked complete:

- database tests proving expired leases cannot create overlapping unfenced
  attempts and reconciliation state survives restarts;
- installer crash/restart tests at every stage boundary, including failure
  between an effect and its receipt, with the same session and no plaintext
  secrets persisted;
- sandbox transport and executor tests proving one stable key reaches Rhai and
  WASM capability calls while guest input cannot forge it;
- effect-owner tests proving duplicate attempts replay the prior effect result
  or are explicitly rejected as unreconciled;
- authorized operator-reconciliation tests with audit/outbox evidence;
- multi-replica PostgreSQL tests for claim fencing and tenant isolation;
- transport tests for stable conflict/uncertain outcome responses.

## Consequences

This decision intentionally trades automatic availability for correctness when
an outcome is uncertain. It requires additive owner schemas, a durable worker
and resumable installer state machine, versioned sandbox context propagation,
capability-owner adoption, operator recovery surfaces, and migration of old
claims. Those gates must be implemented and verified before the full recovery design
can be considered complete. The artifact slice now refuses lease-based
takeover, keeps failed/ambiguous receipts in `reconciliation_required`, and
propagates the stable key through Rhai and WASM capability calls. HTTP installer
jobs now persist their stable session ID before the worker starts, and the
executor creates that exact session. This is deliberately fail-closed but not
a complete recovery system: installer resumability, a versioned remote sandbox
transport contract, effect-owner receipts, and an operator reconciliation
workflow remain open.
