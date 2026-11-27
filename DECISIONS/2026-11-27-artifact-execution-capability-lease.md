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
its identity at each capability preflight. After mixed-closure fences, SQL
admission takes the existing owner shared active-snapshot row lock (SQLite
already owns its write reservation), revalidates the canonical stored hash
and compares both revision and hash. Composition CAS writes conflict with
this lock; restoration of the same manifest cannot renew an old lease.
Missing composition must not be bootstrapped by revalidation. Unrelated
composition replacements conservatively invalidate serving leases.
Lifecycle hooks remain root-only. Serving closures also bind converged
deployment state revision, head IDs, owner-derived readiness digest, node ID
and exact assignment observation revisions for all artifact members. Capture
readiness in one read snapshot and reject a changed head; no selection-only
closure can authorize a serving SQL effect. After mixed-selection fences and
before the composition lock, acquire the existing reconciliation state row
shared lock (SQLite already has its write reservation), compare head/revision,
recompute the persisted digest and check each required live/observed
assignment and observation revision. The ledger owner serializes requests,
claims, heartbeats and reports using the exclusive state row lock. A restored
head cannot revive an older revision. Fresh preflight compares the same
evidence, not a tenant policy digest. This does not fence independent compiled
registry changes, contextual channel/maintenance,
full effective-policy revision or continuous worker/node liveness. PostgreSQL
cross-owner lock-order and runtime evidence remain required. Mixed-closure
tenant intent and composition identity fencing require no additional
migration; coordinate writers and readers.

Registry release authority is independent of emergency security revision.
Migration 000096 introduces a permanent monotonic witness for each registry
(slug, version), including release absence. Row insert, delete, status change
and checksum change advance it atomically; unchanged metadata, rejected writes,
transaction rollback and idempotent replay do not. Publication, yank and unyank
take the platform activation fence before release/request row locks. The database
mutation trigger also takes that fence. Direct SQL maintenance must use the
same order; unrestricted administrative DDL, TRUNCATE and tampering with the
witness are outside this runtime contract. Existing active/yanked/unlisted
eligibility is retained; unavailable status denies admission. A fresh lease
may admit a yanked release but an earlier lease is invalidated by the mutation.
The witness is checked for the exact root and every artifact in the serving
dependency/co-requisite closure, including non-SQL preflight. SQL admission
checks it after sorted platform/tenant slug fences and before readiness and
composition locks; lifecycle remains root-only. No lock is held across guest
execution. Stop old writers and drain leases before migration and rollout;
downgrade would discard continuity evidence and is rejected. PostgreSQL
cross-owner ordering and execution tests remain required.

## Context, events and migration

Tenant, actor, phase, binding, release/installation, capability, execution ID,
trace and idempotency identity remain bound. Channel and maintenance policy
inputs still require owner wiring. Locale, currency and timezone do not
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
