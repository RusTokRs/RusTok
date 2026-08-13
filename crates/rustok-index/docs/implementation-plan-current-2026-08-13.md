# `rustok-index` Current Implementation Plan — 2026-08-13

Status: `cross_milestone_runtime_evidence_admission_pending`

This document supersedes `implementation-plan-current-2026-08-12.md` as the active execution cursor for `rustok-index`.

## 1. Rechecked baseline

The current repository baseline for this cursor is `main@50a8aeb8684f402f35d597469e7a0eb9cf6aaede` on 2026-08-13.

The latest commit touching `crates/rustok-index` on that mainline remains #3471, squash-merged as `77cb554c35cead2f24765b971ef3431d114ef3eb`. No later mainline commit changes the Index crate. #3471 restored the Index core dependency boundary, synchronized `Cargo.lock`, repaired stale Index verifier/documentation assumptions and preserved the M5 Product refresh runtime-evidence gate.

The exact #3471 source head passed:

- `Index Contract CI` #93;
- `Index Storage Smoke Evidence` #3056;
- `Index Storage Scale Evidence` #860.

Those source/compile/storage checks are admission evidence for the merged code. They are not a substitute for the external PostgreSQL/Iggy Product refresh evidence described below.

## 2. M5 Product refresh runtime boundary remains open

The canonical Product/ProductVariant refresh source, typed delivery, host consumer and external-service harness remain source-complete. The dedicated maintainer workflow remains:

```text
.github/workflows/index-product-refresh-redelivery-evidence.yml
```

At this cursor recheck, the workflow still has no retained executions. The remaining M5 boundary is operational:

1. configure operator-approved evidence-scoped PostgreSQL and Iggy GitHub secrets;
2. dispatch `Index Product Refresh Redelivery Evidence` with confirmation `execute` against a reviewed source SHA;
3. require a real successful execution without a source-friendly skip;
4. retain the exact workflow run id, source SHA and result;
5. only then promote the evidence contract/plan out of `runtime_execution_pending`.

The required proof remains unchanged: durable PostgreSQL apply before injected ACK failure, same-offset/raw-envelope redelivery after Iggy restart, durable inbox duplicate admission, ProductVariant progression after successful ACK, and behind-source redelivery without inbox persistence.

Do not infer runtime completion from compilation, source verifiers, workflow source, or PR CI.

## 3. M6 source boundary recheck

The repository already contains the M6 source contracts for bounded replay, locale-scoped replay identity, sealed source continuation, lease/retry/dead-letter handling, graceful interruption, bounded pending futures, drift diagnosis, reconciliation, repair/recovery and retained PostgreSQL evidence packets.

The presence of these documents and source guards is not new work after #3471. Rechecking them against current main shows no independent source-only M6 slice that should be invented merely because M5 external execution is unavailable.

In particular:

- locale predicates are applied by the current Product source before keyset pagination;
- durable locale replay scope, checkpointing and command transport are already source-defined;
- replay lease maintenance, retry/dead-letter transitions and graceful host shutdown are already source-defined;
- drift/reconciliation diagnosis, repair reservations/receipts and prepared-command recovery are already source-defined;
- retained PostgreSQL packets remain maintainer execution/admission work where their documents say execution is pending.

Production partition lifecycle/cutover remains closed until its retained PostgreSQL evidence requirements are satisfied. No source-only shortcut may weaken that gate.

## 4. M7 Product graph/readiness/convergence recheck

The canonical roadmap is already at M7 source maturity even though the 2026-08-12 active cursor was focused on the independent M5 external runner.

Current M7 source state is intentionally single-current and fail-closed:

- Product publishes one current Product Index contract on routing key `4`;
- ProductVariant and SalesChannel use their current selected contracts;
- `PostgresIndexSchemaReadinessStore` requires exact persisted active tenant schema contracts;
- Product-to-SalesChannel durable membership, freshness witness and Channel identity generation are source-complete;
- automatic Product visibility / Channel identity convergence is source-complete through generic `ModuleWork` scheduling;
- the materialized/query freshness fence rejects stale source-read -> mutation-apply windows before user filter/order/cursor/limit/count semantics;
- Storefront parity, public projection, Product-owned tag hydration, serving-budget classification and timeout enforcement remain owner-first/non-serving evidence paths;
- mounted Storefront traffic remains owner-native.

The M7 documents explicitly leave PostgreSQL execution/admission, collation parity, promotion/restart evidence, convergence/identity-transition evidence, timeout evidence and eventual serving composition pending. Therefore this cursor does not create another compatibility schema, bypass readiness, or mount Index into authoritative Storefront traffic.

## 5. Current execution order

The next permitted progress is evidence-driven rather than speculative source expansion.

### Gate A — M5 external Product refresh evidence

Run and retain the manual PostgreSQL/Iggy redelivery workflow described in section 2.

### Gate B — retained M6 PostgreSQL evidence

Execute/admit the source-ready M6 packets required by their individual contracts. Source fixes are appropriate only when those executions expose a real contract or implementation defect.

### Gate C — retained M7 PostgreSQL/runtime evidence

Execute/admit, in dependency order, the current source-ready evidence for:

1. tenant schema readiness and current-schema promotion/restart;
2. Product materialized/query freshness including delayed mutation and locale deletion;
3. Product visibility / Channel generation convergence;
4. Channel create/delete/tenant-move/delete-recreate identity transitions;
5. linked-target/query parity and Product graph restart behavior;
6. Storefront core/EAV/collation parity;
7. deterministic serving-budget timeout/cancellation behavior.

Only after the required readiness, equivalence, freshness, convergence and restart evidence is admitted may a real tenant stage/rebuild/promote sequence be considered. Authoritative Storefront traffic cutover remains last, and owner-native channel-less/deep-page branches remain fail-closed by design.

## 6. Source-work policy while evidence is pending

Until an owner-run evidence packet fails for a source reason, do not create progress by:

- adding legacy/v2/fallback Product refresh routes;
- restoring lower Product schema implementations as runtime fallbacks;
- adding a Product-specific DLQ protocol where generic retry/dead-letter ownership already exists;
- bypassing persisted schema readiness or relation freshness;
- filtering partition/locale scope after pagination;
- widening replay/query budgets to make evidence pass;
- moving mounted Storefront traffic from source inspection alone;
- treating a skipped external-service test as retained evidence.

A failed retained run may justify a focused source repair PR. A missing run does not.

## 7. Agent capability boundary at this recheck

The available GitHub integration can read workflow history but does not expose `workflow_dispatch` or Actions-secret configuration. Therefore this cursor records the exact operational boundary instead of fabricating a runtime result.

The correct continuation for an implementation agent is:

1. recheck the reviewed main SHA and evidence history;
2. if new retained evidence exists, inspect it and implement only the next admitted/failing boundary in a fresh PR;
3. if evidence is still absent and dispatch remains unavailable, leave source semantics unchanged and hand the execution step to the repository maintainer/operator.

## 8. Admission for this cursor update

This revision is documentation-only. It changes no Rust code, schema contract, migration, workflow, verifier, replay behavior, query semantics, relation semantics or Storefront routing.

The admission requirement is therefore:

```text
git diff --check
```

Normal repository CI may run because of repository-wide workflow triggers; unrelated baseline failures must not be represented as Index regressions. The next semantic Index change must again use the focused checks appropriate to the files and boundary it modifies.
