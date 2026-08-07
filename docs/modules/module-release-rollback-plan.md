---
id: doc://docs/modules/module-release-rollback-plan.md
kind: cross_module_implementation_plan
language: en
status: proposed
---

# Module Release and Rollback Plan

## Purpose

Provide a WordPress-like operator experience for production module updates:
an operator can select an update, see its progress and diagnostic evidence,
and receive an automatic safe rollback when the candidate cannot start or
become healthy. The implementation is not WordPress-like: it operates on
immutable source, artifact, admission, and deployment identities rather than
overwriting files in a plugin directory.

This plan establishes the cross-module readiness work. The production release,
activation, rollback, audit, outbox, artifact retention, and rollout mechanism
remain owned exclusively by `rustok-modules`.

## Scope

Included:

- production release identity and predecessor retention for dynamic artifacts
  and static/native module compositions;
- update preflight, sandbox evidence, rollout health evidence, automatic safe
  rollback, manual rollback, and incident diagnostics;
- module data compatibility, migration classification, snapshots, and recovery
  requirements;
- module-local readiness declarations, verification, and staged adoption.

Excluded:

- Next.js build, deployment, rollback, and health automation. Next.js remains
  an optional, manually operated frontend concern owned by its host;
- automatic database restoration after an irreversible migration;
- a second module lifecycle, a module-local rollback implementation, mutable
  artifact replacement, or a runtime fallback to a registry.

## Production Versioning Model

Production versioning is the foundation of update and rollback. A release is
not identified by a human version label alone. It is an immutable record
binding the applicable source identity, dependency lock, build and test
evidence, artifact digest, admission and policy evidence, and the relevant
runtime executor facts.

The control plane separately retains:

1. the selected release for an installation or static composition;
2. the direct predecessor eligible for rollback;
3. desired rollout identity; and
4. observed node health and activation evidence.

No update overwrites source archives, admitted artifact bytes, or a previous
release. Dynamic artifacts execute only from their admitted platform bytes.
Static/native promotions rebuild a complete immutable composition from their
exact inputs. A production update and a rollback are audited transitions
between these identities.

## Safety Model

### Update Mode and Migration Policy

An update has one of two operator-visible modes. These planning labels are not
another version or lifecycle family; canonical type and field names are chosen
only during owner-contract design under the Naming Contract.

| Mode | Entry condition | Update and rollback behavior |
| --- | --- | --- |
| `automatic` | No data change, or a backward-compatible expansion whose exact predecessor and dependency closure can still run | Automatic rollout and one automatic rollback are permitted during the observation window. Data is never restored automatically. |
| `maintenance` | Compensation, irreversible conversion, destructive cleanup, incompatible schema change, or unproven predecessor compatibility | The update requires an explicit maintenance/recovery procedure. Automatic rollback is unavailable once its compensating or irreversible checkpoint begins. |

Migration policy remains a separate owner declaration with the existing
`reversible`, `compensating`, and `prohibited` values. A `reversible` policy
does not by itself make an update automatic: the owner must also prove that the
predecessor and its dependency closure remain compatible with the live data.
The updater must expose the selected mode before the operator starts the
operation. It must never present a maintenance update as an ordinary one-click
update.

### Safe Update Path

1. Resolve one exact admitted candidate and retain its direct predecessor.
2. Verify immutable build, admission, policy, artifact, and complete enabled
   dependency-closure facts for both the candidate and direct predecessor.
3. For a dynamic artifact, execute bounded scenarios through the neutral
   `rustok-sandbox` contract against fixtures or an isolated data copy. Alloy
   may author or test a draft through that shared sandbox, but Alloy does not
   own production activation, rollback, or database migration.
4. For a static/native composition, run the equivalent isolated composition
   build and test evidence; it does not claim WebAssembly sandbox isolation.
5. Apply only a backward-compatible schema expansion, if required.
6. Start the desired rollout and collect bounded readiness and health evidence.
7. Mark the serving rollout successful only after the required health
   conditions pass. Immutable release selection and observed serving state
   remain distinct owner facts.
8. Retain the predecessor for the configured rollback window. Destructive
   cleanup is not part of this path.

An automatic rollback uses the same typed, authorized, audited owner command
as a manual rollback. It is allowed only when the direct predecessor remains
admitted, unrevoked, retained, and compatible with the current data state.
The operation records one rollback attempt only; a failed return to the
predecessor leaves the module stopped or degraded and requires an operator
instead of oscillating between releases.

### Failure and Incident Policy

Automatic rollback may react only to deterministic update-window signals:

- candidate startup, admission, capability, binding, or dependency failure;
- failed sandbox preflight or failed rollout readiness;
- bounded crash, trap, timeout, or server-failure thresholds during the
  observation window; or
- rollout deadline expiry before the required healthy node set converges.

An isolated application error after a completed observation window creates an
incident and preserves the manual rollback path; it does not silently revert a
release. A revoked predecessor is never an automatic target.
The exact health-threshold policy is captured during preflight and reused for
the operation; duplicate signals cannot create another automatic rollback.

Each update, rollback, stop, or recovery-required outcome records one
correlation identity linking the release identities, actor or automation
reason, migration checkpoint, snapshot reference when present, sandbox/build/
admission evidence, rollout observations, and redacted bounded diagnostics.
Operator surfaces show the reason and the correlation identity. Full logs must
remain access-controlled, bounded, and free of secrets and raw sensitive data.

### Minimum Operator Functionality

The shared owner status projection must show the selected release, direct
predecessor, candidate, update mode, current rollout outcome, rollback
eligibility, and the incident correlation identity when one exists. The shared
command surface must support update, observation of the current operation,
manual rollback, and disable/stop. It must not expose direct release-pointer,
artifact-byte, database-restore, or registry-mutation operations.

## Data and Migration Policy

The platform must not promise a universal database rollback. The normal design
is to avoid needing one.

1. A failed transactional migration before commit leaves no committed schema or
   data change.
2. A release eligible for automatic code rollback must keep the direct
   predecessor compatible with the expanded schema and data representation.
3. Backfills run as resumable, idempotent owner operations with durable
   checkpoints; they do not hold a database transaction across sandbox or
   worker execution.
4. Destructive cleanup, irreversible conversion, and incompatible constraints
   are a later finalization step after the rollback window closes. Each
   temporary compatibility bridge requires an owner and a removal condition in
   the affected module plan.
5. After an irreversible checkpoint, the updater must disable automatic
   rollback. A failure moves the module to a controlled stopped or read-only
   state and creates a recovery-required incident.
6. Snapshot restoration is separately authorized. It never automatically
   overwrites live production data because writes after the snapshot would be
   lost.

Module-scoped recovery is possible only where the module has an explicit data
ownership boundary and a tested consistent snapshot/restore procedure. Where
that boundary does not exist, the fallback is platform-level PostgreSQL
recovery, not a falsely scoped module rollback.

## Module Readiness Contract

Every stateful module must add a concise `Release and Data Rollback Readiness`
block to its existing local implementation plan. It records:

- runtime kind: dynamic artifact, static/native composition, or none;
- data owner and storage boundary: none, brokered artifact data, or native
  schema;
- migration classification: reversible, compensating, or prohibited;
- automatic or maintenance update mode and the evidence for that choice;
- direct-predecessor compatibility statement and the evidence that proves it;
- dependency-closure compatibility statement for an automatic update;
- snapshot and recovery requirement, including the boundary that can actually
  be restored;
- backfill/checkpoint behavior, if applicable;
- rollback-window close condition and owner of any temporary compatibility
  bridge; and
- required verification: `N -> N+1 -> N` for eligible releases, or an explicit
  recovery-required assertion for an irreversible migration.

Stateless modules do not need a local section. They receive a `not_applicable`
entry in the central readiness board. Module owners do not add local rollback
services, direct registry reads, arbitrary DDL, or a second release ledger.

## Work Plan

### 1. Record the Architectural Decision

- [x] Record the [release safety ADR](../../DECISIONS/2026-08-06-module-release-rollback-safety.md), defining update modes, the automatic-rollback
  boundary, and the non-automatic database recovery boundary.
- Cross-reference the existing artifact rollback, artifact-data snapshot, and
  neutral sandbox ADRs without replacing their ownership decisions.
- Update `rustok-modules` control-plane documentation with the final owner
  contract.

### 2. Build the Readiness Inventory

- Classify every module as stateless, brokered artifact data, or native schema.
- Identify module data owners, cross-module foreign-key/order dependencies,
  current migration behavior, and restore feasibility.
- Add a central readiness board in `docs/modules/registry.md`; do not claim a
  module is rollback-ready without recorded evidence.

### 3. Complete the Owner Mechanism

- Extend only `rustok-modules` with the typed update preflight, update mode,
  health-window evaluation, incident receipt, and automatic rollback policy.
- Keep desired rollout and observed node state distinct. A failed rollout must
  not be represented as a successful activation.
- Reuse the existing immutable release, predecessor, CAS, audit, outbox,
  revocation, and idempotency boundaries. Do not add mutable artifact or
  version-family paths.
- Expose one operator-facing status projection and command surface through the
  existing owner transports and CLI; frontend hosts remain consumers, not
  owners.

### 4. Prepare Modules in Waves

- Start with stateless and brokered-data modules to prove the safe path.
- Prepare native-schema modules with additive migrations and resumable
  backfills before enabling automatic rollback for them.
- Treat modules with cross-module data ownership or irreversible changes as
  maintenance-only until their recovery boundary is designed and verified.
- Keep Next.js outside the readiness decision; a module owner may record a
  manual Next.js rollout note, but it is neither an automatic gate nor a
  rollback target in this plan.

### 5. Verify and Rehearse

- Test candidate rejection before activation, failed startup, failed readiness,
  stale command, duplicate command, revoked predecessor, failed predecessor
  return, and rollout timeout.
- Test automatic-eligible `N -> N+1 -> N` transitions against PostgreSQL for every module
  that claims automatic rollback.
- Test that an irreversible checkpoint denies automatic rollback and creates a
  recovery-required incident without changing live data.
- Rehearse at least one module-scoped recovery only after its data boundary and
  restore procedure are explicit; separately rehearse platform PostgreSQL
  recovery.
- Retain evidence through the existing audit, outbox, and operations paths.

## Completion Conditions

This plan is complete when:

- every module is classified in the central readiness board;
- every stateful module has its local readiness block and evidence status;
- `rustok-modules` is the sole production owner of update, rollback, incident,
  and release-selection state;
- automatic updates support a verified automatic rollback with operator-visible
  diagnostics;
- irreversible changes cannot silently enter the automatic rollback path; and
- the documented recovery boundaries have been rehearsed rather than inferred.

## Related Documents

- [Module artifact rollback boundary](../../DECISIONS/2026-07-13-module-artifact-rollback-boundary.md)
- [Durable artifact-data snapshot and guarded restore](../../DECISIONS/2026-07-22-artifact-data-snapshot-restore.md)
- [Neutral sandbox foundation](../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md)
- [Module control-plane consolidation plan](./module-control-plane-consolidation-plan.md)
- [`rustok-modules` implementation plan](../../crates/rustok-modules/docs/implementation-plan.md)
