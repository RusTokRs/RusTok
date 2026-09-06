# Implementation recheck — targeted drift repair — 2026-08-06

Status: `source_reviewed_unvalidated_owner_composition_pending`.

## Reviewed scope

- `crates/rustok-index/src/application/drift_repair.rs`
- `crates/rustok-index/src/application/drift_repair_tests.rs`
- `crates/rustok-index/src/infrastructure/postgres/drift_repair.rs`
- `crates/rustok-index/src/migrations/m20260806_000007_add_index_finding_repair_commands.rs`
- application, PostgreSQL, crate, and migration exports
- targeted-repair, lifecycle, persistence, confirmation, README, and live-plan documentation
- targeted-repair and downstream static guards

## Application boundary

Source review confirms:

- tenant, finding, command, target, actor, and reason are required and bounded;
- target identity and source/absence versions are positive and tenant-consistent;
- authorization runs before storage and mints a private-constructor capability;
- store, evidence reader, and owner consume the authorized capability;
- one target-kind owner is selected from a duplicate-rejecting registry;
- execution order is reserve, before evidence, one owner call, after evidence, terminal completion;
- repaired outcome requires after evidence and owner receipt digests;
- denied, not-started, repaired, not-repaired, and already-completed outcomes are typed;
- failures expose only retryable/permanent classification and bounded machine code;
- command and capability debug output omit actor subject and reason text.

The application layer imports no database, SQL, transport, scheduler, or runtime-extension capability.

## Finding commitment parity

The PostgreSQL store does not trust the typed target merely because it was authorized.

For the exact open finding it re-derives and compares:

- entity scope;
- missing or orphan check name;
- orphan identity suffix;
- deterministic finding key;
- expected evidence digest;
- actual evidence digest;
- fixed details contract marker.

The recheck compared the missing/orphan domain tags and component order with
`drift_confirmed_candidate_writer.rs`. The repair store intentionally adds a separate target-kind tag
only to its private command payload digest; it does not add that tag to finding identity or evidence
formulas.

This prevents a caller from substituting another source identity, link name, ordinal, target,
locale, indexed source version, or admitted absence version.

## Reservation transaction

`reserve` uses PostgreSQL `SERIALIZABLE READ WRITE` and:

1. advisory-locks exact tenant plus command UUID;
2. validates exact replay or rejects command UUID payload drift;
3. locks the exact finding row;
4. requires current state open for a new reservation;
5. validates typed finding commitment;
6. checks for another prepared command;
7. inserts one prepared row.

The migration partial unique index on tenant/finding where state is prepared is the database
backstop for one active command per finding.

An exact prepared command resumes only while its exact finding commitment remains admissible. A
completed command returns its stored receipt. No arbitrary prepared row can be claimed through a
ticket because the ticket binds tenant, command, finding, and payload digest.

## Evidence and owner boundary

The generic service requires a bounded evidence reader and one owner per supported target kind.
Source review confirms no default evidence reader, no default repair owner, and no allow-all
authorizer are registered.

Owner calls receive the durable command UUID through the authorized command and are required by the
documented contract to be idempotent. This is necessary for a crash after owner mutation but before
after evidence or terminal receipt persistence.

The service records `Repaired` only when before evidence is repairable, the owner returns an applied
receipt digest, and after evidence is converged. Other admitted results become `NotRepaired`.

## Completion transaction

`complete` uses another PostgreSQL `SERIALIZABLE READ WRITE` transaction and:

- takes the same tenant/command advisory-lock namespace as reservation;
- locks and validates the exact prepared row;
- returns an existing completed receipt idempotently;
- reads the exact finding under a share lock;
- downgrades completion to `NotRepaired(finding_not_open)` if lifecycle state changed;
- updates only terminal receipt columns and database completion timestamp.

The migration trigger allows only an identity-preserving `prepared -> completed` update. Completed
rows cannot be rewritten through the same table contract. Finding state, finding evidence, lifecycle
audit, Index entities, and Index links are not mutated by the repair store.

## Migration review

Migration `m20260806_000007_add_index_finding_repair_commands`:

- rejects MySQL before DDL;
- supports PostgreSQL and SQLite schema creation;
- uses a composite foreign key to the exact finding;
- bounds actor, reason, owner, outcome, and digest columns;
- checks prepared versus completed row shape;
- installs one-active-command partial unique index;
- installs identity-preserving completion triggers;
- preserves existing finding/tenant cascade retention.

No migration scenario was executed.

## Known fail-closed boundary

A prepared reservation intentionally survives retryable dependency or process failure.

If lifecycle closes the finding and the original execution reaches completion, the terminal receipt
is `NotRepaired(finding_not_open)`. If the process crashes before completion and retry can no longer
reconstruct admissible evidence from the closed finding, the prepared reservation remains ambiguous
and fails closed. Expiry, abandonment, operator recovery, and lifecycle-vs-repair coordination are
not silently invented in this slice.

## Exclusions confirmed

The reviewed diff adds no:

- concrete evidence reader;
- concrete missing/orphan mutation owner;
- finding resolution after repair;
- public GraphQL, HTTP, CLI, MCP, or native-admin transport;
- `ModuleRuntimeExtensions` insertion;
- scheduler, worker, automatic finding iteration, or page loop;
- shadow, full, or automatic repair;
- raw record payload, SQL cause, actor subject, or reason in public outcomes/failures.

## Validation disclosure

No tests, Node verifiers, formatting, Cargo checks, migrations, PostgreSQL/SQLite scenarios,
workflows, or CI were executed. Compile, migration-runtime, concurrency-runtime, owner-idempotency,
and end-to-end repair behavior are not claimed.
