# Implementation Plan for `rustok-pages`

## Current state

`rustok-pages` owns pages, bodies, blocks, menus, visibility, and the page
publish pipeline. Its admin and storefront packages use the module-owned
core/transport/Leptos split. Storefront keeps both the native server-function
and GraphQL selected paths; neither UI package has a legacy `api.rs` facade.

Pages is the reference consumer of the `grapesjs_v1` Page Builder capability.
Its manifest fixes the consumer version, capability set, typed disabled states,
and four fallback profiles. `pages-wave0-dry-run-evidence.json` is explicitly
synthetic and the Wave 1 packet is a readiness draft, so neither is production
rollout evidence.

Legacy blocks path works in read/bridge mode: an initial import is supported,
while visual-builder body writes preserve blocks and do not add a block-update
surface. `verify-page-builder-pages-legacy-bridge.mjs` locks that behaviour.

## FFA/FBA status

- FFA status: `in_progress` — module-owned admin and storefront surfaces are
  present and must keep the core/transport/UI boundary.
- FBA status: `in_progress` — Pages has the reference consumer metadata and
  static fallback coverage, but no observed tenant control-plane evidence.
- Evidence: `scripts/verify/verify-pages-ui-boundary.mjs`,
  `crates/rustok-page-builder/contracts/evidence/pages-wave0-dry-run-evidence.json`,
  and `crates/rustok-page-builder/contracts/evidence/pages-wave1-readiness-draft.json`.

## Open results

1. Run Wave 0 against an internal tenant and replace the synthetic evidence
   packet with observed before/after flag and health snapshots, smoke results,
   metrics, traces, and owner decision. Done when the packet is accepted by
   the evidence and correlation gates without placeholder values.
   Dependency: a runnable Page Builder control plane. Verification:
   `node crates/rustok-page-builder/scripts/verify/verify-page-builder-wave-evidence-packet.mjs`
   and `node crates/rustok-page-builder/scripts/verify/verify-page-builder-correlation-evidence.mjs`.
2. Promote the reference consumer through a real Wave 1 only after the Wave 0
   result and provider persistence/rendering paths are verified. Done when an
   approved tenant packet proves `preview -> properties -> publish(dry)`, all
   fallback profiles, rollback execution, and the correlation
   `builder write -> pages publish -> storefront read`.
   Dependency: Page Builder provider readiness and owner sign-off.
   Verification: `npm run verify:page-builder:consumer:pages` and
   `npm run verify:page-builder:wave1-readiness-draft` until the draft is
   replaced.
3. Decide and execute the legacy-blocks exit policy. Done when the owner has
   recorded the supported tenant migration path, removal preconditions, and
   the outcome for the bridge without silently deleting existing blocks.
   Dependency: an inventory of legacy block consumers and content migration
   approval. Verification: `npm run verify:page-builder:pages:legacy-bridge`
   plus targeted page round-trip tests.

## Verification

- `npm run verify:pages:ui-boundary`
- `npm run verify:page-builder:consumer:pages`
- `npm run verify:page-builder:pages:legacy-bridge`
- `npm run verify:page-builder:wave1-readiness-draft`

## Boundaries

- Pages owns page/menu lifecycle, visibility, published reads, and migration
  safety for its existing blocks.
- Page Builder owns GrapesJS capability delivery, feature flags, persistence
  and rendering adapters, and control-plane rollout mechanics.
- Hosts compose Pages UI packages and do not take ownership of page policy or
  Page Builder internals.
