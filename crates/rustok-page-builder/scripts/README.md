# Scripts

This folder stores scripts that are specific to this crate/module.

Rules:

- Keep module-specific verification, migration, generation, or maintenance scripts here.
- Keep cross-platform orchestration scripts in the repository-level `scripts/` folder.
- When script behavior changes public/runtime contracts, update local docs and central docs accordingly.

## Verification scripts

- `verify/verify-page-builder-fba-baseline.mjs` runs the provider/consumer anti-drift gate.
- `verify/verify-page-builder-contract-registry.mjs` compares `contracts/page-builder-fba-registry.json` with provider/consumer manifests and fails on version/capability drift.
- `verify/verify-page-builder-fallback-matrix-docs.mjs` keeps central, provider, and pages fallback matrix docs aligned with `all_on`, `publish_off`, `preview_off`, and `builder_off` runtime profiles.
- `verify/verify-page-builder-wave-evidence-template.mjs` validates `contracts/page-builder-wave-evidence-template.json`, the machine-readable Wave 0/Wave 1 evidence packet template for metadata, control-plane snapshots, fallback smoke, observability, rollback and owner approvals.
- `verify/verify-page-builder-wave-evidence-packet.mjs` validates `contracts/evidence/pages-wave0-dry-run-evidence.json`, the synthetic `pages` Wave 0 dry-run packet that exercises all baseline toggle profiles before real tenant evidence is attached.
- `verify/verify-page-builder-error-catalog-binding.mjs` verifies that provider metadata, `rustok-pages` consumer metadata, the FBA registry, and the `PagesError` runtime catalog use the same typed builder error semantics (`validation`, `sanitize`, `runtime`, `feature-disabled`).

- `verify/verify-page-builder-runtime-fallback-gate.mjs` runs the provider runtime fallback tests used by the CI baseline gate.
- `verify/verify-page-builder-pages-fallback-gate.mjs` runs the `rustok-pages` service fallback checks plus admin/storefront host-helper static checks for all baseline profiles (`all_on`, `publish_off`, `preview_off`, `builder_off`).
- `verify/verify-page-builder-next-admin-parity.mjs` keeps Next Admin page-builder save errors aligned with the `rustok-pages` typed error catalog and operator-guidance contract.
- `verify/verify-page-builder-leptos-admin-parity.mjs` keeps the module-owned Leptos admin package aligned with the same typed error catalog and localized operator-guidance contract.
- `verify/verify-page-builder-flutter-parity.mjs` keeps Flutter mobile shared app-core error mapping aligned with the same typed page-builder catalog and operator-guidance contract.
