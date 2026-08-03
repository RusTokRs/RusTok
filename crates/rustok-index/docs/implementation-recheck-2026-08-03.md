# `rustok-index` implementation recheck — 2026-08-03

## Audited baseline

- Repository: `RusTokRs/RusTok`
- Target branch: `main`
- Audited commit: `e66540bceffe0ae23ee2d04e0f39a1a6ab08aaeb`
- Validation owner: repository maintainer

The source tree, current canonical plan, recent Index pull-request history, and still-open
PostgreSQL evidence pull requests were rechecked. Tests, verifiers, formatting, Cargo commands,
PostgreSQL execution, workflows, and CI were not run, per maintainer instruction.

## Corrected merged state

The canonical plan understated several source-complete M5/M6 slices already present on
`main`:

- source-versioned mutation event acknowledgement substrate;
- production source-call timeout classification;
- bounded no-write replay dry-run;
- cooperative replay page interruption safe points;
- reconciliation retry transition storage and runner wiring;
- failed-scope dead-letter admission, bounded inspection, and authorized requeue;
- generic bounded reconciliation host scheduling ownership;
- bounded drift-finding inspection and persistence for already-computed digest mismatches.

Those capabilities remain narrower than their former combined checklist bullets. In
particular, cooperative interruption is not yet bound to the active PostgreSQL runner lease,
dry-run does not provide targeted/full/shadow rebuild modes, and scheduling does not establish
retained multi-host or graceful-shutdown evidence.

## Continued slice

This branch adds an environment-gated PostgreSQL lifecycle harness for the drift-finding
writer. It uses the real Index migrations and independent PostgreSQL connections to retain:

- advisory-lock serialization for one tenant/finding key;
- one stable finding identity across create and refresh;
- exact bounded entity-scope and digest storage;
- resolved-to-open reopen semantics;
- ignored-state suppression preservation;
- tenant-separated deterministic finding identities.

The harness is source-ready only. No execution or retained evidence is claimed until the
repository owner runs and admits it.

## Remaining open work

- produce and compare authoritative source/index digests under a defined snapshot or watermark;
- diagnose missing entities, stale entities, and orphan links without unbounded ID collection;
- bind cooperative interruption to active runner cancellation and deadlines;
- add targeted/full/shadow rebuild and repair modes;
- add finding resolve/ignore commands with actor/reason audit and fail-closed authorization;
- retain before/after repair, retry/dead-letter, scheduler, restart, and multi-instance evidence;
- add locale/partition checkpoint dimensions, graceful shutdown, and authorized transports;
- complete incremental acknowledgement evidence, persisted per-tenant readiness, durable
  Product-to-SalesChannel relation evidence, consumer cutover, query equivalence, and partition
  admission execution.

## Next implementation cursor

Define the bounded producer contract that computes one authoritative source/index digest pair
for an exact tenant/schema/entity scope and calls the existing writer only after both digests
are available under the same admitted consistency boundary. Keep repair and lifecycle commands
separate from diagnosis.
