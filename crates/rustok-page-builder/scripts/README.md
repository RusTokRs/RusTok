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

- `verify/verify-page-builder-runtime-fallback-gate.mjs` runs the provider runtime fallback tests used by the CI baseline gate.
- `verify/verify-page-builder-pages-fallback-gate.mjs` runs the `rustok-pages` service fallback checks plus admin/storefront host-helper static checks for all baseline profiles (`all_on`, `publish_off`, `preview_off`, `builder_off`).
