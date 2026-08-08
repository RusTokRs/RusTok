# M7 Product Storefront serving-budget policy

Status: `policy_source_complete_timeout_enforcement_pending`.

## Why `PortContext.deadline_ms` is not enough

`PortContext.deadline_ms` carries the original duration budget for a port call. It is not an absolute deadline
and does not automatically decrease while the authoritative Product owner read executes.

A future Storefront Index router therefore must not treat `deadline_ms` as the remaining time available for
Index execution and Product post-page hydration. At the post-owner handoff it must provide a monotonic,
host-measured `remaining_ms` observation.

## Explicit host policy

`ProductStorefrontIndexServingBudget` carries host-selected positive phase budgets:

- `index_execution_ms`;
- `tag_hydration_ms`;
- `safety_margin_ms`.

The constructor rejects zero Index/tag phases and checked-add overflow. This source slice deliberately does not
choose global production SLO numbers; those values belong to host configuration/admission and require runtime
evidence.

## Classification

`classify_product_storefront_index_serving_budget` receives:

- the request `PortContext` with original deadline semantics;
- an optional configured serving budget;
- `ProductStorefrontIndexServingBudgetObservation` containing host-measured `remaining_ms` and whether the
  required Product tag-hydration capability is selected.

The request remains owner-native when any of these are true:

- original deadline is absent/zero;
- serving budget policy is unavailable;
- post-owner remaining budget was not measured;
- measured remaining budget exceeds the original deadline and is therefore inconsistent;
- Product tag hydration capability is unavailable;
- remaining budget is below `index + tag hydration + safety margin`.

Only an internally consistent observation with enough remaining budget returns `Eligible` and carries the two
phase limits plus safety margin forward to a later enforcement adapter.

## Boundary

This policy does not execute Index queries, call Product hydration, create timers, or switch traffic. The current
non-serving shadow executor is deliberately not changed to consume it. Mounted Storefront remains owner-native.

The next source slice must add a non-serving budgeted execution adapter that actually applies the admitted
Index and tag-hydration phase timeouts. Timeout/error results must remain separate from the already-successful
owner response, and no mounted Storefront cutover may occur in that slice.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
