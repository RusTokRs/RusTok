# M7 Product Storefront budgeted execution

Status: `source_complete_runtime_latency_evidence_pending`.

## Post-owner only

`ProductStorefrontIndexBudgetedProjectionExecutor` receives an already-successful authoritative Product
Storefront page. It does not call `list_filtered_published_products` and cannot replace the owner result.

The adapter starts only with `ProductStorefrontIndexServingBudgetDecision::Eligible`. Any owner-native budget
decision is rejected before the first timeout or Index operation.

## Bounded phases

The admitted Index phase budget is enforced twice:

- copied into the projected `PortContext.deadline_ms` so Product schema/EAV owner capabilities see the same
  bounded phase contract;
- enforced externally with `tokio::time::timeout` around raw `execute_projected`.

Only a successful raw page reaches Product public placeholder projection and Product tag hydration.

The admitted tag phase budget is likewise copied into the Product tag `PortContext.deadline_ms` and enforced
with an outer `tokio::time::timeout` around Product-owned `hydrate_projected_tags`.

The safety margin is retained in the result for evidence/telemetry but is not spent as another phase.

## Failure separation

The result retains:

- authoritative owner page;
- raw projected page or Index-phase timeout/error;
- optional public title/handle projection;
- optional Product tag hydration or tag-phase timeout/error;
- raw identity/count/page comparison;
- admitted phase/safety budgets.

A raw Index failure skips public projection and tag hydration. A tag failure/timeout leaves the successful raw
and public pages intact. None of these outcomes changes the authoritative Product owner result.

## Separation from evidence and serving

The existing owner-first shadow executor remains the unbudgeted evidence path. Its post-owner phase methods are
crate-visible only so this separate adapter can enforce phase budgets around them.

Mounted Storefront does not call the budgeted adapter. Source completeness here does not establish acceptable
latency, timeout cancellation behavior, external-provider deadline propagation or serving readiness. Those
require retained runtime evidence and maintainer execution/admission before any traffic switch.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
