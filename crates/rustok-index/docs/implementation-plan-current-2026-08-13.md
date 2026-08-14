# `rustok-index` Current Implementation Plan — 2026-08-13

Status: `cross_milestone_runtime_evidence_admission_pending`

This document supersedes `implementation-plan-current-2026-08-12.md` as the active execution cursor for `rustok-index`.

## 1. Rechecked baseline

The current repository baseline for this cursor is `main@00080aa368846acb5d71e103ea5a69ae5aa683fe` on 2026-08-14.

The latest mainline change touching the Index implementation is #3518, squash-merged as `1e2846126dc9a79d39c46fde1011c6db2776f4f5`. #3518 rebuilt the reviewed six-file Product current-schema promotion repair directly on fresh main, corrected PostgreSQL `INT4` schema-version decoding in registration/readiness, aligned the retained key4 promotion packet with real Channel migrations and canonical prerequisites, and kept the promotion/parity source verifiers formatting-tolerant without weakening fail-closed boundaries.

The 2026-08-14 recheck inspected intervening mainline changes through `00080aa368846acb5d71e103ea5a69ae5aa683fe`. No later semantic Index implementation change supersedes #3518; the intervening repository changes are outside the Index runtime boundary, so they do not open a new source-only Index slice.

The exact final source head `4e42513937220123937a13473e709815092b8a8c` passed terminal, non-cancelled focused evidence:

- `Index Contract CI` #148;
- `Index Product Current Schema Promotion Evidence` #34;
- `Index Storage Smoke Evidence` #3091;
- `Index Storage Scale Evidence` #877.

The Product promotion workflow included the retained PostgreSQL key4 promotion + restart packet. These results admit the #3518 repair itself and its current-schema promotion/restart sub-boundary. They do not substitute for the independent external PostgreSQL/Iggy Product refresh evidence described below.

Repository-wide failures observed on that PR head were separately traced outside the six-file Index diff, including Browser `sessionStorage` failures, module-manifest documentation drift and a pre-existing Forum migration SQL syntax failure. They are not Index admission evidence and must not be folded into this cursor as speculative Index work.

## 2. M5 Product refresh runtime boundary remains open

The canonical Product/ProductVariant refresh source, typed delivery, host consumer and external-service harness remain source-complete. The dedicated maintainer workflow remains:

```text
.github/workflows/index-product-refresh-redelivery-evidence.yml
```

The 2026-08-14 workflow-history recheck still reports zero retained executions across all branches. The remaining M5 boundary is operational:

1. configure operator-approved evidence-scoped PostgreSQL and Iggy GitHub secrets;
2. dispatch `Index Product Refresh Redelivery Evidence` with confirmation `execute` against a reviewed source SHA;
3. require a real successful execution without a source-friendly skip;
4. retain the exact workflow run id, source SHA and result;
5. only then promote the evidence contract/plan out of `runtime_execution_pending`.

The required proof remains unchanged: durable PostgreSQL apply before injected ACK failure, same-offset/raw-envelope redelivery after Iggy restart, durable inbox duplicate admission, ProductVariant progression after successful ACK, and behind-source redelivery without inbox persistence.

Do not infer M5 runtime completion from compilation, source verifiers, workflow source, PR CI or the separate M7 current-schema promotion packet.

## 3. M6 source boundary recheck

The repository already contains the M6 source contracts for bounded replay, locale-scoped replay identity, sealed source continuation, lease/retry/dead-letter handling, graceful interruption, bounded pending futures, drift diagnosis, reconciliation, repair/recovery and retained PostgreSQL evidence packets.

Rechecking them against current main shows no independent source-only M6 slice that should be invented merely because M5 external execution is unavailable. Retained PostgreSQL packets remain maintainer execution/admission work where their individual contracts say execution is pending.

Production partition lifecycle/cutover remains closed until its retained PostgreSQL evidence requirements are satisfied. No source-only shortcut may weaken that gate.

## 4. M7 Product graph/readiness/convergence recheck

The canonical roadmap remains at M7 source maturity while the independent M5 external runner is still the first open execution gate.

Current M7 source state remains intentionally single-current and fail-closed:

- Product publishes one current Product Index contract on routing key `4`;
- ProductVariant and SalesChannel use their current selected contracts;
- `PostgresIndexSchemaReadinessStore` requires exact persisted active tenant schema contracts;
- Product-to-SalesChannel durable membership, freshness witness and Channel identity generation are source-complete;
- automatic Product visibility / Channel identity convergence is source-complete through generic `ModuleWork` scheduling;
- the materialized/query freshness fence rejects stale source-read -> mutation-apply windows before user filter/order/cursor/limit/count semantics;
- Storefront parity, public projection, Product-owned tag hydration, serving-budget classification and timeout enforcement remain owner-first/non-serving evidence paths;
- mounted Storefront traffic remains owner-native.

#3518 admits the current Product key4 promotion/restart repair packet on its exact final source head. It does not by itself admit the rest of the retained M7 runtime matrix: broader tenant readiness, materialized/query freshness, convergence/identity transitions, linked-target/query parity, Storefront core/EAV/collation parity, deterministic serving-budget timeout/cancellation, or eventual serving composition.

## 5. Current execution order

The next permitted progress is evidence-driven rather than speculative source expansion.

### Gate A — M5 external Product refresh evidence

Run and retain the manual PostgreSQL/Iggy redelivery workflow described in section 2. This remains the immediate execution cursor.

### Gate B — retained M6 PostgreSQL evidence

Execute/admit the source-ready M6 packets required by their individual contracts. Source fixes are appropriate only when those executions expose a real contract or implementation defect.

### Gate C — retained M7 PostgreSQL/runtime evidence

Continue the remaining source-ready evidence in dependency order. The Product current-schema promotion/restart repair packet has fresh terminal evidence from #3518; the remaining runtime boundaries are:

1. any broader tenant schema readiness evidence still required by its contract;
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

The available GitHub integration can read workflow history and repository state but still does not expose `workflow_dispatch` or Actions-secret configuration. A direct 2026-08-14 capability recheck returned no dispatch operation.

The correct continuation for an implementation agent is:

1. recheck the reviewed main SHA and evidence history;
2. if new retained evidence exists, inspect it and implement only the next admitted/failing boundary in a fresh PR;
3. if M5 evidence is still absent and dispatch remains unavailable, leave source semantics unchanged and hand the external execution step to the repository maintainer/operator.

## 8. Admission for this cursor actualization

This revision is documentation-only. It changes no Rust code, schema contract, migration, workflow, verifier, replay behavior, query semantics, relation semantics or Storefront routing.

The admission requirement is therefore:

```text
git diff --check
```

Normal repository CI may run because of repository-wide workflow triggers; unrelated baseline failures must not be represented as Index regressions. The next semantic Index change must again use the focused checks appropriate to the files and boundary it modifies.
