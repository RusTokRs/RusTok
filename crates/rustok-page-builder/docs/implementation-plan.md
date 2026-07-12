# Implementation Plan for `rustok-page-builder`

## Current state

`rustok-page-builder` owns the `grapesjs_v1` capability provider for preview,
tree, properties, and publish. The provider has a versioned registry,
permission map, typed error catalog, fallback profiles, control-plane evidence
contracts, and framework-neutral endpoint adapter seam in `src/adapters.rs`.
`handle_page_builder_graphql_endpoint` and
`handle_page_builder_leptos_server_function_endpoint` delegate through the
canonical request, authorization, policy, and error envelopes.
The transport bridge is source-locked by
`scripts/verify/verify-page-builder-transport-bridge.mjs`.

The provider has adapter extension points and source-locked dry-run evidence,
but no selected persistence/rendering adapter or observed tenant rollout. Pages
is the reference consumer; consumer fallback and migration safety stay owned by
the relevant consumer plan and contracts.
Control-plane dry run evidence defines the required flags, snapshots, decisions,
and waiver policy before a tenant can be promoted.

## FFA/FBA status

- FFA status: `not_started` — this provider has no module-owned UI surface.
- FBA status: `in_progress` — the provider contract, endpoint adapter seam,
  and no-compile evidence are ready; runtime integration and live evidence are
  still open.
- Evidence: `contracts/page-builder-fba-registry.json`,
- Structural shape: `no_ui_boundary`
  `contracts/page-builder-adapter-seams.json`,
  `scripts/verify/verify-page-builder-endpoint-adapters.mjs`, and
  `scripts/verify/verify-page-builder-transport-bridge.mjs`, and
  `npm run verify:page-builder:fba:baseline`.

## Open results

1. Bind a selected persistence and rendering adapter to the provider and wire
   owner-owned GraphQL and Leptos server-function endpoints. Done when preview,
   save, publish, and typed fallback behaviour execute against a tenant-scoped
   adapter without transport-local capability or error aliases.
   Dependency: the chosen persistence/rendering implementation and host
   composition. Verification: `npm run verify:page-builder:fba:baseline` plus
   targeted adapter runtime tests.
2. Replace synthetic Wave evidence with observed tenant control-plane packets.
   Done when Wave 0 and Wave 1 carry correlation from builder write through
   Pages publish to storefront read across required profiles, owner approval,
   and no waiver; Flutter Wave 1 participation also supplies device/runtime
   evidence.
   Dependency: priority 1 and Pages reference-consumer readiness. Verification:
   `npm run verify:page-builder:fba:baseline`.
3. Agree the legacy-block bridge exit with the Pages owner. Done when supported
   migration, removal preconditions, and an owner outcome are recorded without
   deleting existing blocks through builder body writes.
   Dependency: legacy content inventory and Pages migration approval.
   Verification: `npm run verify:page-builder:pages:legacy-bridge`.

## Verification

- `npm run verify:page-builder:fba:baseline`
- Targeted adapter runtime tests after a persistence/rendering adapter is chosen.

## Boundaries

- Page Builder owns capability delivery, provider contracts, endpoint envelopes,
  feature profiles, and rollout mechanics.
- Pages owns page lifecycle and legacy block migration safety; forum remains a
  later consumer of the public capability contract.
- Hosts compose provider endpoints and do not define provider-local contracts.
