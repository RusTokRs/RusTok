# Final cutover gate for transitioning RBAC to `casbin_only`

- Date: 2026-03-05
- Status: Accepted

## Context

The relation graph is already accepted as the canonical source of RBAC data. Before switching the runtime to `casbin_only`, the platform must go through a controlled parity window and formalize a Go/No-Go package, so that the cutover does not depend on implicit agreements.

## Decision

1. **Transition to `casbin_only` is only allowed after a full pre-cutover gate.**
   - Staging rehearsal is complete and invariant artifacts are attached.
   - The baseline helper `scripts/rbac_cutover_baseline.sh` shows:
     - `rustok_rbac_engine_mismatch_total` delta = 0
     - `rustok_rbac_shadow_compare_failures_total` delta = 0
     - decision volume >= `min-decision-delta`
   - The auth release gate is closed and attached to the release bundle.

2. **Rollback gate is mandatory and defined before the switch.**
   - Immediate rollback: revert to `RUSTOK_RBAC_AUTHZ_MODE=casbin_shadow`.
   - Rollback triggers:
     - increase in 401/403 beyond baseline;
     - anomalous deny-rate without expected business context;
     - degradation of authorization latency above the agreed SLO;
     - increase in `rustok_rbac_shadow_compare_failures_total` or other authz-path incident indicators after the switch.

3. **Release evidence is mandatory.**
   - `artifacts/rbac-staging/*`
   - `artifacts/rbac-cutover/*`
   - auth release-gate report
   - gate decision summary (`Go`/`No-Go`) with responsible roles

## Consequences

### Positive

- Cutover to `casbin_only` becomes reproducible and auditable.
- The decision relies on parity evidence, not an intuitive assessment of readiness.

### Trade-offs

- Formal release overhead is added.
- Discipline is required for collecting and storing operational artifacts.

### Follow-up actions

1. Attach actual cutover artifacts to the nearest release window.
2. After stable `casbin_only`, close cleanup tails and update steady-state runbooks.
