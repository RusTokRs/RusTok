# Bounded execution through the shared migration runner

- Date: 2026-11-05
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-migrations, rustok-modules
- Extends: 2026-11-03-migration-consolidation, 2026-08-06-module-release-rollback-safety
- Supersedes: None
- Superseded by: None

## Context

The PostgreSQL `StepRunner` verifies each committed journal prefix, but its
pending range extends to the end of the compiled composition. A caller stopping
its loop is not an executor-enforced phase boundary. Passing a subset instead
loses the predecessors required to validate the journal. The installer also has
a deliberately sparse bootstrap selection, which is not a composition prefix.

## Decision

Extend the existing runner with `ExecutionPhase`: the complete ordered
`expected_prefix` followed by a nonempty ordered `steps` list. Validate both
lists against the beginning of the complete compiled composition before
connecting. Steps must be contiguous; omitted or reordered identities fail.
Retain the complete composition for journal verification.

Under the existing session-bound writer lock, require an exact journal prefix
ending between the phase's start and end, inclusive. This permits restart after
committed steps and an explicit already-completed retry. A database before the
prerequisite boundary or beyond the terminal boundary fails before installing
the journal or executing a migration. Never apply suffix steps. All selected
phase steps require the supported transactional recovery policy; prerequisites
and suffix steps are not executed by that phase.

Expose the same mechanism through `rustok-migrate up --phase <json-file>` for
operator and fixture execution. Bare `up` retains full-plan behavior. A phase
file is an untrusted execution restriction, not release authorization or a
safety receipt.

## Sources of truth and ownership

`rustok-migrations` owns phase validation, SQL execution and journal
reconciliation. The compiled composition remains the executable inventory; a
phase cannot supply code or a second ordering. `rustok-modules` retains
production update admission, fencing, exact source/build/schema identity,
durable recovery and finalization policy.

## Invariants

No phase can skip a prerequisite, include an unlisted step, expand to latest,
accept a sparse journal or execute past its terminal boundary. Failure or
cancellation poisons the same runner. Completion means the selected journal
boundary was verified, not that the entire schema is current or a module update
is accepted. Rejected boundaries perform no journal installation.

## Non-goals

This prerequisite does not implement native production authorization, source
sealing, observed-schema admission, owner SQL confinement, durable operation
receipts, nontransactional recovery or consolidation admission. Names alone
cannot establish source or database equivalence. Installer bootstrap is unchanged.

## Data, transaction, and concurrency boundary

Reuse the single non-reconnecting PostgreSQL session and advisory lock. Every
selected step uses its declared transaction behavior. Check the complete journal
before and after each committed step, including completion. The lock excludes
cooperating runner sessions, not arbitrary writers or the entire fleet. SQL
effects remain owned by each migration and are not confined by its identity.

## Context dimensions

The phase is database-wide operational input, not tenant or module authority.
No channel, locale, actor or release authority is inferred from names.
Production composition must verify its owner-defined authority before execution.

## Events and projections

Committed-step confirmations remain journal evidence for existing verification
consumers. No lifecycle, activation or rollback event is invented. CLI
completion distinguishes a phase from a full-plan run.

## Failure semantics

Malformed, empty, unknown-field, mismatched or noncontiguous requests fail
closed. Unknown, duplicate or sparse journal identities fail. An out-of-phase
database is not advanced to satisfy the request. Error, cancellation or session
loss requires closing and reopening; resume revalidates the same phase against
committed history. Preserve execution and cleanup errors together.

## Migration and cutover

Existing full-plan callers use the same implementation. The CLI is the current
phase consumer. Native production integration remains pending until its owner
can provide authorized inputs and durable recovery. Preserve installer bootstrap
and all published migration history.

## Alternatives considered

A second executor duplicates locking and recovery. Truncating the vector hides
predecessors. A terminal identity alone cannot express the intended predecessor
or detect omitted intermediate steps. Treating operator JSON as approval would
bypass the module owner's release and schema-safety contract.

## Verification

Use scoped parser tests and isolated PostgreSQL execution. Cover exact stops,
restart, completed retry, missing prerequisites, history beyond the end,
omitted/reordered identities, unsupported recovery, interruption, competing
writers and journal drift. Assert data/journal state and absent suffix effects.

## Consequences

Installation, consolidation fixtures and future native updates share one
execution mechanism. Explicit boundaries remove caller-loop dependence without
claiming the still-missing production admission and recovery evidence.
