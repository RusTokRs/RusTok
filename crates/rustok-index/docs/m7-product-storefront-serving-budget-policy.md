# M7 Product Storefront serving-budget policy

Status: `policy_and_timeout_enforcement_source_complete_runtime_evidence_pending`.

## Remaining budget contract

`PortContext.deadline_ms` is the original duration budget, not a decreasing remaining deadline. A future
Storefront Index router must measure `remaining_ms` monotonically after the authoritative Product owner call.

`ProductStorefrontIndexServingBudget` carries host-selected positive `index_execution_ms`,
`tag_hydration_ms` and `safety_margin_ms`. The constructor checked-adds the phases and rejects zero required
phases/overflow. No production SLO values are hard-coded in this source.

`classify_product_storefront_index_serving_budget` keeps the request owner-native when the original deadline,
configured policy, host-measured remaining budget or required Product tag capability is unavailable, when the
observation is inconsistent, or when remaining budget is below the checked phase total. Only then can it return
`Eligible` with the admitted phase limits.

## Non-serving timeout enforcement

`ProductStorefrontIndexBudgetedProjectionExecutor` is a separate post-owner adapter. The caller supplies the
already-successful authoritative `StorefrontProductList`; the adapter never repeats the owner list read.

It accepts only an `Eligible` decision. A non-eligible decision returns `BudgetNotEligible` before projected
work starts.

For an eligible decision the adapter:

1. narrows a cloned `PortContext.deadline_ms` to `index_execution_ms`;
2. wraps the raw `execute_projected` phase in `tokio::time::timeout` with the same budget;
3. derives public title/handle placeholders only from a successful raw page;
4. narrows the Product tag context to `tag_hydration_ms`;
5. wraps Product-owned tag hydration in a second `tokio::time::timeout`;
6. retains raw identity/count/page comparison only when the raw Index phase succeeded.

Index timeout, raw projection error, public projection error, Product tag error and tag timeout remain separate
outcomes. None can replace or mutate the already-successful authoritative owner result.

The existing `ProductStorefrontIndexShadowExecutor::execute` remains the unbudgeted owner-first evidence path.
Only its crate-private `execute_projected` and `hydrate_projected_tags` phases are reused by the budgeted adapter.
It does not consume serving-budget decisions or start timers itself.

## Boundary

Mounted Storefront remains owner-native and does not call the serving-budget policy or budgeted executor.
This source proves only the structure of timeout enforcement, not production latency or timeout behavior.
Maintainer-run runtime evidence is required before any traffic-switch adapter can be admitted.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
