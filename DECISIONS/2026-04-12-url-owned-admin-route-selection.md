# URL-owned route selection for module-owned admin UI

- Date: 2026-04-12
- Status: Accepted

## Context

RusToK admin surfaces had drifted into multiple selection patterns:

- local component state used as the primary selection source;
- ad hoc query parsing in individual module packages;
- mixed query key styles such as generic `id`, camelCase keys, and package-specific names;
- silent auto-select-first behavior that hid invalid or missing selection state.

That approach does not scale across `apps/admin`, `apps/next-admin`, simple editors, and composite dashboards such as `ai`, `forum`, and `channels`. It also makes deep links unstable and allows stale editor state to survive failed reloads.

## Decision

RusToK adopts one platform contract for module-owned admin UI:

- selection state is URL-owned and is the source of truth;
- only typed snake_case query keys are allowed for admin route selection;
- no legacy query-key compatibility layer is kept for `id`, camelCase keys, or similar aliases;
- missing or invalid selection keys resolve to empty state instead of auto-select-first fallback;
- nested selection keys are valid only when their parent selection key is present and compatible.

Ownership is split explicitly:

- `rustok-api` owns typed admin route-selection schema, sanitization rules, and invariants;
- `leptos-ui-routing` owns only generic Leptos route/query plumbing and stays reusable for admin and storefront;
- host apps own adapters and parity:
  - `apps/admin` provides the Leptos route policy and URL writer behavior;
  - `apps/next-admin` implements the same schema-level contract through local Next helpers.

Module-owned admin packages consume host-provided route state and must not invent package-local route-selection contracts.

## Consequences

- Deep links become deterministic and portable across admin hosts.
- Invalid selection no longer produces hidden fallback behavior or stale detail state.
- The platform gains a single route-selection audit target instead of multiple local conventions.
- Existing legacy links using `id` or camelCase keys break by design and must be regenerated with canonical snake_case keys.
- New admin packages must document and test typed route-selection behavior as part of their done definition.
