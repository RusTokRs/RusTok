# Shared migration safety assessment

- Date: 2026-11-05
- Decision status: Accepted
- Implementation status: Implemented
- Owners: rustok-core, rustok-migrations, rustok-modules
- Extends: 2026-11-03-migration-consolidation, 2026-08-06-module-release-rollback-safety
- Supersedes: None
- Superseded by: None

## Context

The schema-operation classifier and module preflight describe migration safety
independently. Flattening a maintenance result into a boolean and optional
diagnostics can lose the rejection: preflight previously selected automatic mode
when an unsafe migration supplied no reason text. Downstream consumers also
checked different projections of the same preflight result.

## Decision

`rustok-core::MigrationSafetyAssessment` is the shared migration fact contract.
It distinguishes additive-safe, maintenance-required and unknown assessments.
Diagnostics explain an assessment; an empty explanation cannot change its state.
Locking and destructive facts remain attached to maintenance assessments.

`rustok-migrations` classifies supplied schema operations into this contract.
Its summary and projected schema digest remain diagnostics, not observations of
PostgreSQL or authority to execute a production change. Unclassified SQL and
missing operation coverage cannot assert additive safety.

The limited `AddColumn` summary does not prove a safe lock duration or default
expression. PostgreSQL requires `ACCESS EXCLUSIVE` for that operation and a
volatile default can rewrite the table; the summary therefore requires
maintenance even for nullable/defaulted columns. See the
[PostgreSQL 16 ALTER TABLE contract](https://www.postgresql.org/docs/16/sql-altertable.html).
Concurrent index creation separately requires nontransactional recovery evidence.

`rustok-modules` derives the same fact from admitted dynamic descriptors and
adds settings, external-effect and data-copy restrictions. Its preflight receipt
provides one conservative automatic-eligibility predicate. Transition admission
and data-upgrade evaluation use that predicate rather than independently trusting
a mode or boolean. The coordinator additionally binds a receipt to the exact
operation and module.

## Sources of truth and ownership

Foundation owns the assessment vocabulary, alongside `MigrationSource` and
migration phase/safety metadata. The migration owner owns SQL execution and
catalog observation. The modules owner owns update and recovery decisions.
Receipts are projections of those owner decisions. Dependency direction remains
`rustok-migrations -> rustok-modules -> rustok-core`; no reverse import is added.

## Invariants

### Allowed states

An explicit additive-safe assessment may pass the migration gate. Automatic
eligibility also requires compatible settings evidence and no owner denial.
Maintenance and unknown facts always prevent automatic admission, even without
diagnostic text. A receipt must match its transition's operation and module.

### Forbidden states

Unknown evidence must not default to safe. A forged automatic mode cannot
override unsafe facts, missing settings protection or recorded denials. Source
equivalence, schema summaries and successful builds cannot grant live mutation
authority.

## Non-goals

This change does not implement native production phase authorization, release
sealing, consolidation replacement admission, a universal executor, persistence
scaffolding, database restoration or complete fleet conflict exclusion.

## Data, transaction, and concurrency boundary

The assessment is pure data and has no database transaction. Existing owner
transactions, revision checks, idempotency, activation locks and checkpoints
remain authoritative. The coordinator rejects mismatched receipts before
creating a transition. An assessment never acquires a maintenance fence.

## Context dimensions

Preflight binds operation and module identity plus source/target schema and plan
digests. Tenant, principal, policy, trace and topology stay in the owner command
and checkpoint; they are not reconstructed by foundation. Locale, timezone and
currency are not applicable to the assessment.

## Events and projections

Existing owner outbox and checkpoint effects are unchanged. Diagnostic mode and
reason fields cannot become independent authorization sources.

## Failure semantics

Maintenance, unknown or contradictory evidence denies automatic progression.
Unknown serialized variants and missing required assessment fields fail decoding.
Uncertain database execution still requires the existing executor reconciliation
contract; this assessment cannot infer completion.

## Migration and cutover

Replace the internal boolean/reason input and local classification enum
atomically across actual callers and tests. Preflight bodies are not stored in
durable transition checkpoints; those retain their existing schema. No adapter
reconstructs safe evidence from an old boolean. Existing migration files and
identities are not rewritten. Changed foundation/build inputs still require
their normal release replay evidence; this contract does not exempt them.

## Alternatives considered

Adding a fallback reason fixes one symptom but retains incompatible facts.
Importing `rustok-migrations` into the modules owner creates a dependency cycle.
Merging developer filesystem recovery with production recovery combines distinct
authorization and transaction boundaries. A new crate or execution engine is
unnecessary for the two existing consumers.

## Verification

Exercise maintenance and unknown assessments with empty diagnostics, locking-only
DDL, raw SQL, absent settings evidence, owner side effects, contradictory
receipts and foreign operation/module identities. Verify real dynamic transition
and data-upgrade consumers as well as serde rejection. Run scoped foundation,
migration classifier and modules tests plus the architecture dependency guard.
These checks do not establish PostgreSQL equivalence or production deployment
readiness.

## Consequences

The shared contract removes one unsafe translation between migration facts and
release policy. Native phase execution, supported predecessor comparison,
consolidation admission and DB-aware scaffolding remain explicit later gates in
their existing owner plans.
