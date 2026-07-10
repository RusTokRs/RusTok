# Implementation plan for `leptos-shadcn-pagination`

## Current state

`leptos-shadcn-pagination` owns presentation-only Leptos pagination primitives:
container/content/item/link/previous/next/ellipsis markup, active state, and
basic accessibility attributes. It does not own page calculation, data fetching,
route/query policy, or domain list behavior.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `shared_ui_support`
- This presentation support crate is not a module-owned FBA provider.

## Open results

1. **Remove package-local pagination copy.** Require host-provided localized
   previous/next content or a host-owned label contract; do not retain English
   defaults or add package-local i18n fallback chains.
   **Depends on:** the shared UI i18n/host composition contract.
   **Done when:** pagination labels use the host effective locale and every
   consumer can supply accessible localized content.

2. **Validate reusable accessibility and interaction markup.** Cover active,
   disabled, href, `aria-current`, `aria-disabled`, and ellipsis semantics in
   focused component tests and consuming hosts.
   **Depends on:** agreed presentation/accessibility expectations.
   **Done when:** primitives render stable semantic markup without embedding
   page-navigation policy.

3. **Keep pagination policy outside the primitive crate.** Add props only for
   reusable presentation; retain page calculation, route/query writing, data
   fetching, and domain-specific labels with the host or owner package.
   **Depends on:** demonstrated cross-surface presentation reuse.
   **Done when:** the public component API remains generic and consumers do not
   need local forks for their navigation policy.

## Verification

- Focused component tests for active, disabled, and accessibility markup.
- Host/module UI tests for route/query and localized label composition.

## Change rules

1. Do not add data fetching, page calculation, route/query, or i18n ownership.
2. Update the local README with a changed component/accessibility contract.
3. Update consumers if component props or markup semantics change.
