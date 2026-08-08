# Pages / Page Builder shared contribution tooling actualization — 2026-08-08

Status: `source-ready / execution-pending`

## Scope

This slice continues the Pages/Page Builder contribution parity sequence after PR #3215. The Pages reference consumer already stored its Fly contribution declaration in canonical `crates/rustok-pages/rustok-module.toml`, but parsing, normalization and provider-version injection still lived inside the Pages admin build script. That left the metadata shape reusable in data but not yet reusable as module tooling or publish-readiness policy.

## Shared source boundary

`crates/rustok-module-manifest` is now the lightweight shared parser/normalizer for module contribution metadata. It intentionally depends only on `serde`, `serde_json` and `toml`; it does not depend on `fly-ui`, Leptos, Page Builder runtime packages or the `rustok-modules` control plane.

The shared normalizer:

- reads `[module].slug` and `[module].version` as module/owner identity;
- reads `[fba.builder_consumer].capabilities` as the declared contribution capability envelope;
- reads `[fba.builder_consumer.contribution_manifest]` with exact target-provider versions, dependencies, permissions and separate admin/storefront contribution arrays;
- rejects contributions whose required capability is outside the declared builder-consumer capability envelope;
- rejects undeclared target providers and owner-version conflicts;
- requires nested renderer/property-editor provider identity to match the contribution provider;
- reserves `ownerProvider` and `providerVersion` for generation and injects those fields from canonical module metadata;
- removes optional build/export-only `role` metadata before producing the runtime manifest JSON;
- returns `None` for modules that do not declare contribution metadata, so adoption remains explicit rather than mandatory for unrelated modules.

The shared crate is metadata tooling only. It does not build a runtime `ContributionRegistry`, choose tenant policy, observe provider health, own persistence, or introduce a second Fly authority.

## Pages build-time consumer

`crates/rustok-pages/admin/build.rs` is reduced to a Pages-specific adapter over the shared normalizer. It retains only Pages-specific assertions and exported constant names:

- module slug must remain `pages`;
- module version must match the admin crate package version;
- `landing_blocks` and `metadata` role exports must remain on the admin surface;
- landing blocks must target an exact external provider version;
- metadata must remain owned by the Pages provider and expose one property editor;
- the normalized shared manifest is emitted into `OUT_DIR` for the unchanged Pages runtime helper API.

TOML parsing and generic contribution normalization are no longer Pages-owned.

## Publish readiness

`xtask module validate <slug>` now invokes the same shared contribution normalizer while validating a publishable module package. When contribution metadata exists, publish readiness additionally requires:

- normalized module id/version to agree with the package manifest;
- admin contributions to have `[provides.admin_ui]`;
- storefront contributions to have `[provides.storefront_ui]`.

Generic provider/version/capability admission is shared with the build-time consumer rather than duplicated inside `xtask`.

## Guardrails

The Fly contribution guard now requires the shared crate, its lightweight dependency boundary, Pages build-time reuse and `xtask` publish validation. It rejects reintroduction of the old Pages-local parser/normalizer. The Pages metadata guard likewise follows canonical module metadata through shared normalization into the generated runtime manifest.

## Boundaries retained

This slice does not change Pages metadata/document persistence, reviewed publication, immutable artifacts, rollback/repair, route/cache ownership, authenticated authoring or public rendering. It does not add a runtime TOML dependency. It does not connect or fabricate Page Builder SLO observations; missing live health remains `unobserved`.

No tests, Cargo checks, Node verifiers, formatting, TOML parser execution, builds, workflows, CI or browser/database evidence were run by the implementation agent.

## Next source cursor

The repository currently has only the Pages production `fba.builder_consumer.contribution_manifest` consumer. The next contribution source slice should select a second production consumer only after its persistence/authorization/preview ownership is explicit, then make that consumer use this shared module metadata boundary rather than creating a consumer-local schema or generator.

Provider-health observation and maintainer-owned execution evidence remain separate open cursors.
