# Durable module node reconciliation ledger

- Date: 2026-08-14
- Status: Accepted

## Context

The module control plane already persists native-distribution rollout state, but
dynamic artifact and sandbox readiness is supplied as an ephemeral host
snapshot. A cache, sandbox-worker readiness response, or process-local registry
cannot prove that every selected installation has been prepared and observed on
the nodes that may execute it.

Dynamic artifacts have tenant and installation-specific capability scopes. The
node reconciliation owner must therefore preserve the exact admitted
installation identity without moving module governance, SQL access, or secret
resolution into `rustok-sandbox` or a deployment agent.

## Decision

`rustok-modules` owns one durable module-node reconciliation ledger for dynamic
artifact and sandbox assignments. The ledger uses the shared
`ModuleDesiredObservedState`, `ModuleReconciliationPhase`,
`ModuleReconciliationEvidence`, and `ModuleReconciliationFailure` contract;
it does not introduce a second desired/observed or node-phase family.

Each owner-selected assignment binds one node to the exact installation ID,
installation scope, admitted release digest, payload digest, payload kind,
admitted payload media type, dependency-graph revision and digest,
capability-grant revision, executor ABI, and policy revision. Desired
sets are produced by the control-plane owner and persist before a node agent is
asked to act. Node agents can claim only their own assignment under a bounded
lease and receive no other-node observations, database access, capability
grants, secret handles, or mutable release selection.

The ledger persists desired and observed heads, revisioned assignment reports,
idempotency receipts, and failure evidence in the same transaction as its
outbox events. Baseline reports use the shared `pending`, `prepared`,
`healthy`, `active`, and `failed` phase contract. The owner alone admits
aggregate activation and observed-head convergence after every required
assignment is healthy or active; stale, divergent, and expired-lease reports
fail closed.

Native distribution rollout remains a distinct aggregate because its immutable
role bundle, direct predecessor, and release-head transition are different
authoritative objects. It shares the reconciliation vocabulary but neither
aggregate reads or mutates the other's state.

`rustok-sandbox` remains a neutral execution foundation. Sandbox-worker and
server transports are authenticated adapters to this owner ledger; they do not
become another source of desired state or derive installation identity from a
slug, latest release, or Alloy workspace.

## Consequences

- Effective policy may consume only owner-verified durable node observations;
  a missing, stale, or mismatched assignment denies affected artifact serving.
- Artifact lifecycle, security, capability-policy, and admitted-release changes
  must create a new owner-selected desired set rather than mutate node-local
  cache entries.
- Deployment agents retain materialization and health execution, while the
  owner retains topology selection, assignment fencing, convergence, audit, and
  transactional outbox publication.
- The initial implementation must include durable schema, owner service,
  idempotent request/claim/report paths, and focused failure/replay tests before
  server transport cutover claims readiness.

## Related Documents

- [Module control-plane consolidation plan](../docs/modules/module-control-plane-consolidation-plan.md)
- [Module release rollback safety](./2026-08-06-module-release-rollback-safety.md)
- [Exact installation identity for sandboxed module artifacts](./2026-07-17-sandbox-artifact-installation-identity.md)
- [Neutral sandbox foundation](./2026-07-11-neutral-sandbox-foundation.md)
