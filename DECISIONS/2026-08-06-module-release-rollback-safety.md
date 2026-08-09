# Module release rollback safety

- Date: 2026-08-06
- Status: Accepted, amended on 2026-08-09
- Supersedes in part:
  - the rebuild-on-rollback rule in
    [Static Promotion Review Boundary](./2026-07-22-static-promotion-review-boundary.md);
  - caller-selected migration compatibility in the current artifact rollback
    command; and
  - any direct operator rollback path owned by `rustok-build`.

## Context

RusToK needs an operator-friendly production module update experience: users
must be able to identify the serving, candidate, and previous releases; start
one update; understand rejection or failure; and return to a verified direct
predecessor when that is safe.

The platform is a compiled, distributed application with dynamic artifacts,
not a plugin directory whose files can be overwritten. Dynamic rollback is
scoped to an installation. Native code, the server, embedded Leptos surfaces,
generated registries, and browser assets are deployed as a complete compiled
distribution and cannot be rolled back as isolated files.

The repository currently has overlapping static release state:

- `rustok-build` exposes an active release and direct rollback mutation;
- `rustok-modules` owns the platform composition projection;
- `rustok-modules` also owns static-distribution release and desired/observed
  rollout state.

The `rustok-build` rollback changes database statuses and emits an event but
does not prove deployment or health convergence. The static-distribution
rollback queues a new build, which makes compiler, worker, source CAS,
toolchain, signing, and verifier availability part of the incident path.
Neither is the required bounded automatic recovery contract.

Existing control-plane contracts retain immutable artifacts, predecessor
lineage, audit/outbox facts, migration checkpoints, artifact-data snapshots,
security state, and desired/observed native rollout facts. They do not justify
automatic restoration of committed database data. Sandbox, build, migration
transaction, or rollout evidence alone cannot prove that the predecessor
remains correct after live writes or an irreversible effect.

## Decision

### Canonical Owner and Release Units

Production versioning, update safety, rollback eligibility, and incident
outcome are one `rustok-modules`-owned lifecycle. A production release is an
immutable identity binding source, dependency lock, build/test evidence,
artifact/UI and role digests, declared migration/data-contract facts, and
executor identity. Immutable admission evidence is attached to the release
record, but current policy/security revisions, topology, controller authority,
node observations, and deployment receipts belong to an update/rollout
operation. Changing those live facts does not create a new artifact release.

`rustok-modules` is the sole operator-level owner of:

- module update intent and exact preflight decision;
- selected/serving/candidate/direct-predecessor facts;
- rollback unit, compatibility and observation policy;
- durable operation, complete cross-scope conflict fence set, and one
  automatic attempt;
- rollback selection, desired/observed rollout, retention hold, and incident
  outcome; and
- the shared operator projection and command result.

`rustok-build` owns canonical role-plan construction/validation and shared
non-operator build primitives. `rustok-static-distribution-worker` is the sole
trusted executor/publisher for an owner-authorized complete static role bundle
and returns one canonical static role-bundle receipt. There is no second static
publisher. `rustok-migrations` remains the neutral trusted migration executor.
Sandboxes and deployment agents execute narrow owner-authorized work. None owns
a second update, rollback, or incident decision.

The atomic implementation cutover removes the public `rustok-build` active
release/rollback mutation, its duplicate mutable head, and every direct
GraphQL/native/CLI caller. No dual-write or compatibility path is retained.

The release and rollback unit is:

- one exact platform- or tenant-scoped installation for a dynamic artifact; if
  dependency resolution changes other installations, the complete changed
  lock graph joins the unit; or
- the complete immutable role distribution for static/native code. The
  recovery operation separately binds its exact topology snapshot.

A static rollback can therefore return every module co-released in the direct
predecessor composition. Operator preflight shows that complete blast radius.
Unchanged dynamic dependencies and active dependents are eligibility evidence,
not mutation targets.

### Static Incident Path

Automatic static recovery redeploys the retained immutable direct-predecessor
role bundle. Before candidate rollout begins, the owner must prove that all
required server, worker, embedded Leptos, generated-registry, and browser-asset
bytes and receipts are retained; rehash them; and revalidate current admission, security,
policy, data compatibility, topology, and deployment authority.

Rollback creates a new audited transition but neither edits artifact bytes nor
compiles a replacement. It uses the normal desired/observed rollout reconciler,
and succeeds only when the predecessor role bundle is observed healthy.
Rebuild remains release-admission/reproducibility evidence or a separately
admitted maintenance update through the same owner lifecycle. It is never a
rollback fallback. Missing predecessor bytes or evidence makes automatic mode
ineligible.

### Executable Transition Decision

Update mode is computed for one exact predecessor-to-candidate update and its
candidate-to-predecessor recovery within one live scope. Documentation,
semantic versions, a sandbox run, a module
declaration, or caller input cannot authorize it.

`rustok-modules` persists an immutable decision bound to the releases, rollback
unit, dependency and active-dependent closure, configuration/data/schema and
migration checkpoint, security/policy/topology revisions, health policy,
retention, recovery evidence, and evidence digests. The owner reloads these
facts under revision/fence checks before every state-changing transition.
Missing, stale, contradictory, or unverifiable evidence fails closed into
maintenance.

Preflight requires no conflicting nonterminal operation and converged selected,
desired, and observed-serving state across the conflict set. It returns an
immutable preview of mode, mutation unit, blast radius, denial/eligibility
reasons, rollback-window effect, migration/point-of-no-return facts, fences,
and recovery action. Apply binds that exact receipt; changed evidence requires
a fresh preview. Static composition and maintenance updates require explicit
confirmation.

The current caller-selected `migration_rollback_mode` authority is removed.
Migration policy retains the existing `reversible`, `compensating`, and
`prohibited` values as owner evidence, but `reversible` is necessary rather
than sufficient.

An update has one operator-visible mode:

- **Automatic** is available only when the direct predecessor and exact
  dependency closure remain admitted, unquarantined, unrevoked, retained, and
  compatible with every intermediate live state. A bounded observation window
  permits one candidate-attributed recovery attempt. Data is never restored.
- **Maintenance** applies to unproven compatibility, non-transactional or
  mixed-fleet-incompatible DDL, compensation, destructive cleanup,
  irreversible conversion, unsafe durable work, or unsafe external side
  effects. Maintenance never performs automatic rollback.

A failure before the desired rollout or any deployment/serving mutation is an
update rejection: predecessor capacity is unchanged and no rollback attempt is
consumed. Once rollout has displaced, stopped, or reduced predecessor capacity,
a candidate startup/readiness failure is a rollout failure and may reserve the
single recovery attempt even before the candidate serves traffic. The
observation window still begins only with the first candidate traffic. An
arbitrary older release is a new admitted update, not rollback.

A later update starts only after the preceding operation is terminal and its
selected, desired, and observed-serving state is converged across the conflict
set. Starting it atomically closes the previous code-rollback eligibility and
establishes the then-serving release as the new direct predecessor. Outstanding
compatibility, finalization, retention, recovery-point, durable-work,
client-lifetime, incident, audit, and legal-hold obligations remain durable
under their owners and are included in the new decision and conflict set. The
new update cannot release or forget them, and an earlier release never remains
a hidden two-hop rollback target.

### Durable Operation and Failure Attribution

One owner operation derives the complete conflict-key set for the rollback
unit, schema/data owners, dependency and active-dependent installations,
topology, and affected namespaces. It acquires or fences that set atomically in
the fixed release-unit, data/migration-owner, namespace, and topology order
before mutation; a scope-local lease cannot authorize a cross-scope change.
The set serializes update, rollback,
disable/deactivate/uninstall, quarantine/revoke,
migration/backfill/finalization, restore/purge, and retention collection. Every
external phase has immutable request binding, monotonic checkpoint, fenced
lease, idempotent terminal receipt, and restart reconciliation. Process or node
loss cannot create a second automatic attempt. Transactional phases use CAS and
idempotency; leases are limited to asynchronous or external work.

Before the first compensating or irreversible effect, the owner durably closes
automatic eligibility and establishes required traffic, job, and write fences.
A crash never reopens that gate. Failure after it creates a
recovery-required outcome.

Maintenance execution proceeds from that gate through the exact authorized
migration/effect and candidate rollout to observed serving health. It is not
accepted at the gate. Any migration, rollout, or health failure after the gate
is recovery-required. Cancellation before the gate is safe; a failed candidate
is never automatically retried and requires a fresh update/preflight.

Trusted observations are fresh and bound to the exact release, rollout scope,
topology, and pinned health policy. Module self-report, ordinary business/input
errors, missing telemetry, or a platform-wide database, broker, network, or
provider outage cannot alone authorize module rollback. A dependency symptom
counts only when a bounded predecessor/control cohort remains healthy and the
pinned policy attributes the regression to the candidate. Quarantine,
revocation, policy, topology, migration, and retention changes preempt stale
decisions. Quarantine/revocation atomically cancels or supersedes a conflicting
stale operation rather than waiting behind its external lease.

A single-node topology cannot use statistical attribution that requires a
control cohort. If trusted telemetry remains missing after candidate traffic
until the pinned deadline, candidate traffic is fenced and the operation runs
its one recovery when eligibility remains proven; otherwise it becomes
recovery-required.

Static recovery authority and its bounded evaluator remain available outside
the candidate application and embedded UI. The deployment controller receives
only the exact operation, candidate, predecessor, topology, policy, deadline,
and single-operation recovery authorization. Reservation/consumption is
persisted atomically: exact same-operation replay resumes idempotently, while a
divergent request or second operation is denied. The controller cannot select
releases, run DDL, or restore data.

### Data, Durable Work, and Finalization

The normal strategy is forward-compatible `expand -> migrate -> contract`, but
automatic mode may rely only on one canonical internal contract. It may leave
additive schema artifacts in place; it must not introduce old/new adapters,
fallback decoders, dual read/write paths, or parallel internal contracts.
Semantic changes that require those mechanisms are maintenance-only.

For automatic mode, both N and N+1 must correctly read, write, validate, index,
and serialize every intermediate database/configuration state. The same
compatibility proof covers public/native transports, artifact bindings, events,
outbox payloads, schedules, queued jobs, retries, caches/indexes, and active
dependents. N+1 work remaining after return to N must be safely consumable,
drained under the bounded authority below, cancelled, or visibly dead-lettered
and reconciled.

Retention never grants execution eligibility. An N+1-pinned work item may run
after rollback only under a bounded item-specific drain authorization that
creates no new work, serves no traffic, revalidates capability, security, and
policy state, and is cancelled by quarantine or revocation. Otherwise the item
is cancelled or visibly dead-lettered for reconciliation.

Code rollback does not undo payments, emails, webhooks, published events, or
other external mutations. Such effects must be compatible, idempotent, fenced,
or covered by a tested reconciliation procedure for automatic mode.

Destructive cleanup is a separate maintenance finalization. Elapsed time is not
authority. Finalization requires an accepted/converged candidate, explicit
rollback-window closure, completed backfills and invariants, no old nodes/work
or incompatible client assets, no incident/recovery/rollback, and satisfied
retention/recovery/legal/audit conditions.

### Recovery Points

Database restoration is a separately authorized recovery operation and never
automatically overwrites live production data.

Module-scoped recovery exists only for an explicit data ownership boundary
with a complete, bounded, tested snapshot/restore procedure. Artifact-data
restore retains its empty-target rule. Cross-module native data normally
requires platform PostgreSQL recovery.

Recovery fences traffic/writes/workers, preserves the failed live state,
restores into an isolated or empty target, verifies identities, security,
domain invariants, objects, projections, outbox/offsets and external effects,
records measured RPO/RTO, and only then performs a separately authorized
cutover. The platform defines no generic merge into live data; any merge is an
owner-specific, tested, separately authorized recovery contract.

### Frontend Boundary

Embedded Leptos server and browser artifacts are part of the static role bundle
and its rollback evidence. Dynamic declarative UI, localization, permissions,
and bindings move with their admitted artifact.

Next.js build/deployment remains optional and manual. It is outside automatic
readiness, health, and rollback and cannot claim success for this lifecycle.

## Consequences

- Every updateable module, including a stateless module, records a local
  release/data readiness block. Stateless modules still account for
  dependencies, durable work, contracts, health, and external effects.
- The central readiness board reports evidence but never grants production
  eligibility.
- The shared operator projection distinguishes selected from serving state,
  shows the rollback unit and blast radius, update mode/reason, observation and
  rollback windows, migration/point-of-no-return facts, fence state,
  eligibility/denial reasons, recovery progress, and sanitized diagnostics.
- Automatic recovery succeeds only after the direct predecessor is observed
  healthy and every required durable-work/external-effect reconciliation is
  terminal. A pointer write, queued build, process launch, or restored traffic
  with unresolved reconciliation is intermediate.
- Manual rollback uses a fresh executable decision, direct-predecessor rule,
  fence set, and convergence definition. Its window begins at candidate
  acceptance, after retention started before rollout, and closes at
  finalization or the next converged update; current configuration, security,
  dependency, data, migration, and retention facts can make it ineligible
  sooner.
- Full logs remain protected, tenant-isolated, bounded, redacted, and separate
  from typed owner receipts and fixed-cardinality metrics.
- Verification includes mixed N/N+1 reads/writes and durable work, process loss
  at every phase, concurrency and security races, external-outage
  non-triggering, retention/GC holds, outside-candidate static recovery,
  irreversible-gate races, tenant isolation, finalization denial, and measured
  restore drills.

## Related Documents

- [Module Release and Rollback Plan](../docs/modules/module-release-rollback-plan.md)
- [Module artifact rollback boundary](./2026-07-13-module-artifact-rollback-boundary.md)
- [Static promotion review boundary](./2026-07-22-static-promotion-review-boundary.md)
- [Durable artifact-data snapshot and guarded restore](./2026-07-22-artifact-data-snapshot-restore.md)
- [Artifact security state boundary](./2026-07-22-artifact-security-state-boundary.md)
- [Shared owner-operation receipt ledger](./2026-08-03-owner-operation-receipts.md)
- [Neutral sandbox foundation](./2026-07-11-neutral-sandbox-foundation.md)
