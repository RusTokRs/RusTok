# Execution-bound host capability authority

- Date: 2026-11-27
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-modules and the server host; Auth/RBAC and individual capability owners
- Extends: [Deployment readiness authority](./2026-11-27-artifact-deployment-readiness-authority.md)
- Supersedes: None
- Superseded by: None

## Context

A valid initial binding admission is not perpetual authorization. A long-running
artifact may attempt capability calls after the original session, role grant,
installation, readiness or capability policy changes. Worker callbacks may run
outside the original task-local request scope.

## Decision and ownership

The server's composed artifact executor registers a non-renewable host-only
lease for each host-generated execution ID. It binds the complete sandbox
subject and context, binding, exact installation and effective policy revision.
The monotonic deadline is bounded by the sandbox wall-clock budget and the
remaining original direct credential lifetime. The bounded in-memory registry
is a lifetime fence, never a durable grant source.

HTTP and command bindings require the captured original direct credential;
the capture shares the request invalidation state and is installed only for
the existing Auth/RBAC transaction when rechecking authority. Background
event/schedule/hook and owner lifecycle/data-upgrade dispatch retain their
own admission contracts; actor attribution cannot become user authorization.
No bearer token, session evidence, or host lease is sent to guest or worker.

## Invariants and concurrency

Before a capability reaches its owner, require a matching active lease, declared
binding capability, current exact installation/grant, tenant policy/readiness
for non-lifecycle execution, and current direct binding permission where
applicable. Recheck lease membership and expiry after asynchronous reads.
Expiry/invalidation denies launch or successful output release; guard drop
removes the execution on completion, error or cancellation. Missing evidence,
mismatched identity or unavailable checks fail closed. No SQL lock or registry
mutex spans external invocation. The original sandbox grant limits and
independent owner effect checks still apply.

For server-composed structured data and event outbox SQL effects, the owner
transaction now takes the existing platform/tenant activation locks after
direct credential/RBAC fences, resolves the current selection and grant
through the canonical owner queries, and compares against the lease's exact
installation and grant constraints. The transaction retains selection locks
through commit, including for non-direct execution. The lease captures the
root release's clear security revision and checks it for every host capability.
SQL effects and security writers share the platform slug fence, including
the implicit clear state. Quarantine/clear does not revive older leases.
Lifecycle admission remains distinct from serving mutations but cannot bypass
release security. Ordinary serving leases also bind the exact artifact dependency/co-requisite
closure validated by readiness. SQL admission locks all artifact slug
fences in ascending slug order (platform then tenant per slug) before
reselecting and verifying the complete artifact set, its durable policies
and clear security revisions. Non-SQL preflight compares fresh closure
evidence without holding effect locks. Owner lifecycle remains root-only.
Artifact selection identity is not continuity evidence: a tenant shadow or
replacement may disappear and restore the same installation. Each artifact
lease binds both scope selection generations on the existing permanent
activation fence rows. The Modules owner advances only the affected scope on
successful activation, deactivation, rollback, uninstall and tenant
enable/disable, atomically with the mutation and receipt. Pure locks,
idempotent replays and aborted transactions do not advance the generation.
Capture generations with exact selected identities in one database snapshot;
compare under the already-held sorted slug locks before SQL effects. Lifecycle
captures its root generation. This conservative generation may invalidate
leases even when a scope mutation does not change the effective winner.
The serving closure also binds every visited module's canonical tenant
lifecycle revision and policy-relevant override intent (including absent
intent); Core continues to ignore overrides. Native and artifact override
commits acquire the existing platform slug fence before the first state
write. Static settings commits already acquire it. SQL admission locks
every visited slug in ascending order (platform then artifact tenant),
checks the revision and current intent, and retains locks through commit.
Outstanding lifecycle claims are not treated as committed intent. No effect
transaction spans guest execution. Settings changes may conservatively
invalidate leases through the shared lifecycle revision.
Serving policy derivation now retains the exact verified active owner
composition revision/hash from which defaults and co-requisites were read.
Reject a changed composition after closure/readiness derivation and compare
its identity at each capability preflight. Before mixed-closure fences, SQL
admission takes the existing owner shared active-snapshot row lock (SQLite
reserves the writer at the owner boundary), revalidates the canonical stored hash
and compares both revision and hash. Composition CAS writes conflict with
this lock; restoration of the same manifest cannot renew an old lease.
Missing composition must not be bootstrapped by revalidation. Unrelated
composition replacements conservatively invalidate serving leases.
Lifecycle hooks remain root-only. Serving closures also bind converged
deployment state revision, head IDs, owner-derived readiness digest, node ID
and exact assignment observation revisions for all artifact members. Capture
readiness in one read snapshot and reject a changed head; no selection-only
closure can authorize a serving SQL effect. After composition and mixed-selection fences, acquire the existing reconciliation state row
shared lock (SQLite already has its write reservation), compare head/revision,
recompute the persisted digest and check each required live/observed
assignment and observation revision. The ledger owner serializes requests,
claims, heartbeats and reports using the exclusive state row lock. A restored
head cannot revive an older revision. Fresh preflight compares the same
evidence, not a tenant policy digest. This does not fence independent compiled
registry changes,
full effective-policy revision or continuous worker/node liveness. PostgreSQL
cross-owner lock-order and runtime evidence remain required. Mixed-closure
tenant intent and composition identity fencing require no additional
migration; coordinate writers and readers.

Registry release authority is independent of emergency security revision.
Migration 000096 introduces a permanent monotonic witness for each registry
(slug, version), including release absence. Row insert, delete, status change
and checksum change advance it atomically; unchanged metadata, rejected writes,
transaction rollback and idempotent replay do not. Publication, yank and unyank
take the platform activation fence before release/request row locks. Migration 000098 splits PostgreSQL database admission from witness mutation.
The canonical platform fence acquires a transaction advisory gate keyed by
`hashtextextended('rustok:module-artifact-platform:' || slug, 0)` before its
permanent activation row. The BEFORE ROW registry guard tries that same gate
without waiting, then locks an existing activation row NOWAIT or creates it
under the gate. UPDATE/DELETE can already hold a release row at this point:
contention aborts the statement with SQLSTATE 55P03 rather than inverting the
blocking order. The AFTER trigger only advances the witness. Identity changes
and TRUNCATE are rejected. SQLite retains its single-writer path. Direct SQL
maintenance spanning multiple slugs must acquire the same gates and activation
rows up front in sorted slug order, and retry a rejected operation only from
a new transaction. Arbitrary prelocked rows, disabled triggers, privileged DDL
and direct witness/fence tampering are outside this runtime contract. Existing active/yanked/unlisted
eligibility is retained; unavailable status denies admission. A fresh lease
may admit a yanked release but an earlier lease is invalidated by the mutation.
The witness is checked for the exact root and every artifact in the serving
dependency/co-requisite closure, including non-SQL preflight. SQL admission
checks it after composition and sorted platform/tenant slug fences and before readiness; lifecycle remains root-only. No lock is held across guest
execution. Stop old writers and drain leases before migration and rollout;
downgrade would discard continuity evidence and is rejected. PostgreSQL
cross-owner ordering and execution tests remain required.

HTTP artifact serving now captures the exact Channel owner resolution and global
durable generation, re-resolving canonical request facts without the TTL cache
and rejecting a different selected channel. A generation read before and after
owner resolution prevents mixed binding snapshots. The existing neutral channel
input enters the mixed artifact policy, including every dependency/co-requisite.
For unresolved HTTP context, retain the generation even without a channel input.
Fresh checks reuse the captured channel input and compare the generation;
structured SQL effects take the owner shared generation lock after composition
and readiness locks. No new migration is required. Channel writes advance the
existing trigger-backed generation; unrelated changes conservatively invalidate
contextual leases. Lifecycle remains owner-only. Deferred transport and
background channel context, and full
cross-owner ordering are not implemented by this change.

## Operational maintenance authority

Migration 000097 introduces a Modules-owned singleton serving-maintenance
state and revision history. It is separate from operations-tool upgrades,
data-copy authorization, and lifecycle intent. Active scopes form a union:
all tenants/all modules, one tenant/all modules, all tenants/one module,
or one tenant/one module. The platform owner replaces the normalized set
with an expected revision through an injected transaction-bound authorizer.
The owner locks the singleton after authorization, advances the revision
only on a changed set, and records the command context and reason atomically.
A no-op does not advance it. Missing or invalid evidence denies serving;
SQL guards prohibit resetting or deleting the singleton.

Artifact serving captures the tenant projection, including an inactive
revision, and evaluates it through the canonical mixed policy. All selected
dependencies and co-requisites participate. The lease retains exact evidence;
fresh capability checks reject changed revisions even after an on/off ABA.
SQL effects retain the shared maintenance row lock after composition and
Channel authority (SQLite writer reservation) until commit. Global revision
changes conservatively invalidate all serving leases. Lifecycle/recovery
admission remains a separate owner path; no maintenance lease is required
to remove the gate. The gate is not proof of quiescence of external effects.

No HTTP/GraphQL/CLI mutation or concrete platform operator authorization
adapter is introduced here. Hosts must not expose the owner replacement API
until that boundary is composed and checked. Native/static serving and
cached operator projections are not wired to this durable gate in this step.
Drain old serving nodes and leases before relying on migration 000097;
downgrade is denied. Direct administrative SQL is not an authorized writer.

## Cross-owner transaction lock order (first integration)

For the Auth/RBAC and artifact serving/security/lifecycle seams examined here,
the canonical order, omitting gates not required by a particular operation, is:

1. active tenant, sorted OAuth app identities, sorted user identities,
   exact credential/session and consent authority;
2. active composition shared row, then tenant lifecycle policy cursor
   where the operation needs static policy authority;
3. sorted role identities and current role-derived authority;
4. sorted module slugs, platform then tenant per slug; selection,
   lifecycle/registry/security evidence under these owner fences;
5. deployment reconciliation state and observation evidence;
6. Channel generation, then operational maintenance revision;
7. effect-specific domain state, receipts and transactional events.

Never discover a new earlier-ranked identity while holding later fences:
abort and restart from the unlocked locator if necessary. Reacquiring an
already-owned slug or cursor is permitted. No new lock may be acquired in a
late recheck; use the transaction's retained owner authority.

Previously static lifecycle commit took the slug and then the lifecycle
cursor, while security administration took the cursor and then the slug.
Lifecycle now locks the cursor and validates the expected predecessor before
the slug on the state-commit path. No-op paths that publish no revision
transition retain only the slug fence. Direct artifact SQL admission now
checks the exact composition after credential locks and before role locks;
background serving acquires composition before the mixed closure. SQLite's
composition owner obtains its own write reservation before reading. Root
grant/security/registry checks finish before Channel and maintenance locks.

Reconciliation request, claim, heartbeat and report writers now acquire the
distinct platform slug fences in lexical order before the reconciliation
state row. Unlocked installation locators are re-read under the fence;
agent paths compare the desired head, state revision and complete target
installation ID set after waiting. If the head or membership changed, reject
rather than acquiring another slug beneath the state lock. SQLite reserves
the writer before reading the desired head. PostgreSQL identity reads use
explicit FOR SHARE OF installation, admission rather than FOR UPDATE on
a joined query: both owner records are read-only, and FK KEY SHARE may
coexist. Installation migration checkpoint writes acquire platform then
tenant slug fences before receipt or identity writes and re-read the locator;
this does not invoke the serving journal guard because a checkpoint may be
part of an admitted lifecycle. Reconciliation revalidates every target's
current installation identity before either transition to Converged, not
merely the installation in the final healthy report.

Reconciliation operation receipts are a private admission fence acquired
before the slug/state locks for request and report; the same receipt is
completed later in the transaction. No inspected path acquires a new receipt
under the state lock. Do not generalize this exception to foreign owner
receipts or FK checks. Holding all target slugs for claim/heartbeat/report
is conservative and can increase contention for large topologies; the
existing bounded topology size limits the set. No schema change is needed.

This is still a partial cross-owner order, not an exhaustive proof.
The registry release AFTER trigger no longer acquires a platform slug fence
after release row locks. The PostgreSQL BEFORE guard and matching canonical
advisory gate reject late competing registry DML, including first insertion.
This closes that particular blocking edge in source, not arbitrary multi-row
or cross-owner deadlocks. RBAC migration 000006 adds nonblocking parent
guards before membership/grant revision triggers and artifact grant/receipt
FK checks, retaining the parent locks through commit. See the membership
and role grant ADRs for the bounded ordering and retry contract. Migration 000099 also fences registry release request references and
publication/yank/unyank receipt FK parents with PostgreSQL KEY SHARE NOWAIT;
publication checks request before release. This avoids waiting on an earlier
parent from a direct child write, retains the FK, and rejects invisible or
missing parents. An unchanged release request reference or SET NULL cascade
requires no new request-parent lock. A 55P03 rejection requires transaction
restart. SQLite keeps its serialized writer path. Other
implicit FK and trigger locks, bulk cascades, owner receipts, RBAC generation
ordering across owners, and external effects need further analysis.
For the governed-transition/security-state seam, the global security epoch
singleton follows the required platform/tenant slug fences and precedes domain
writes. Staging, governed activation and finalization hold a PostgreSQL
FOR SHARE epoch lock through commit; quarantine/revoke acquire FOR UPDATE
before security state writes. SQLite reserves its writer before reading the
epoch. Finalization discovers and revalidates its checkpoint under the platform
fence before acquiring the epoch and reserving the FK-bearing receipt; exact
replay is checked again after waiting. Security operation receipts remain
private admission fences before the slug, with no new receipt acquired below
the epoch. Watchdog and operator failure/recovery paths do not grant new
execution authority and retain their existing revision CAS behavior.
The epoch increment returns its exact successor with UPDATE RETURNING and
rejects exhausted signed-integer storage without mutation. Cross-owner
ordering beyond this slice remains open; emergency mutations may wait for
an already admitted SQL transition to commit, not for external execution.
Migration 000098 preserves existing witness revisions; stop old writers and
drain leases before applying it and deploying the matching gate users.
PostgreSQL contention regressions are authored but not executed.
Coordinate reconciliation and installation writer rollout; do not infer
production deadlock freedom from the authored tests.

## Context, events and migration

Tenant, actor, phase, binding, release/installation, capability, execution ID,
trace and idempotency identity remain bound. Non-HTTP channel policy inputs and the maintenance operator transport still require owner wiring. Locale, currency and timezone do not
participate. The lease is not an event or durable record; existing audit and
idempotency/reconciliation semantics remain unchanged. Migration 000095 adds
a nonnegative selection generation to the permanent activation lock row.
Existing rows and absent rows start at zero; this is a cutover baseline, not
historical continuity evidence. Drain old leases, stop old lifecycle writers,
apply the migration and deploy writers/consumers together; old writers do
not advance the counter. Downgrade is denied. No RPC schema change is needed.
Non-server compositions retain their explicit owner
contracts and are not claimed covered by this lease.

## Failure semantics and non-goals

A pre-launch denial prevents execution. A post-launch denial may follow already
committed guest effects; it is an ambiguous outcome, not authorization to
auto-replay. This decision does not promise cancellation of already-admitted
external calls, atomic domain effect authorization, durable lease recovery,
or immediate interruption of capability-free computation. The next boundary
is owner-transaction effect fencing, including independent service transports.

## Verification

Required: exact context and execution ID mismatch, binding capability absence,
lease expiry and cancellation, captured request invalidation across tasks,
revoked session/role/grant and changed policy/readiness, both Rhai worker and
Wasm paths, owner background/lifecycle compatibility, concurrency and timing.
Source regressions cover scope propagation/invalidation and pre/post launch
lease checks; compilation and the remaining integration evidence are pending.
