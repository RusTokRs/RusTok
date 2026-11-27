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

## Context, events and migration

Tenant, actor, phase, binding, release/installation, capability, execution ID,
trace and idempotency identity remain bound. Channel and maintenance policy
inputs still require owner wiring. Locale, currency and timezone do not
participate. The lease is not an event or durable record; existing audit and
idempotency/reconciliation semantics remain unchanged. No migration or RPC
schema change is needed. Non-server compositions retain their explicit owner
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
