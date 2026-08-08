# Pages / Page Builder Module-Metadata Contribution Generation — 2026-08-08

Status: source-ready / Pages-reference-consumer-complete / shared-tooling-generalization-open / execution-pending

## Rechecked gap

The 2026-08-08 parity reconciliation established that Fly already had separate admin/storefront factories, tenant/permission/capability/policy/health filtering and structural/version diagnostics. The remaining Phase 9 source gap was the Pages reference consumer: `rustok-pages-admin` still hand-authored the same contribution identities, provider targets, capabilities, block ids, messages and metadata that belong in canonical module metadata.

That duplicate declaration is removed in this slice.

## Canonical authority

`crates/rustok-pages/rustok-module.toml` now owns the Pages Page Builder contribution declaration under:

```text
[fba.builder_consumer.contribution_manifest]
[[fba.builder_consumer.contribution_manifest.admin]]
```

The module manifest owns:

- owner provider identity;
- exact target-provider versions;
- contribution roles, ids and providers;
- required Page Builder capabilities;
- contributed Fly block ids;
- messages and contribution metadata;
- the Pages metadata property-editor id/component type/accessibility contract;
- the complete `page_builder_consumer_properties_v1` property schema and field limits.

Owner version is derived from `[module].version`; it is not repeated in contribution metadata.

## Build-time generation boundary

`crates/rustok-pages/admin/build.rs` parses the canonical module manifest at build time and fails closed when:

- module slug/version do not match the Pages admin package;
- builder capability metadata is empty or duplicated;
- a contribution role is missing or duplicated;
- a contribution targets a provider without an exact declared version;
- landing blocks do not target an external version-pinned provider;
- metadata does not target the Pages owner provider;
- the metadata contribution does not expose exactly one property editor;
- contribution metadata tries to hand-author `ownerProvider` or `providerVersion`.

The generator injects `ownerProvider` and exact `providerVersion`, serializes the normalized `ModuleContributionManifest`, and emits stable Rust constants plus one compact JSON manifest into `OUT_DIR`.

`toml` is a build dependency only. Pages admin/WASM runtime does not parse `rustok-module.toml`.

## Runtime boundary

`crates/rustok-pages/admin/src/contributions.rs` now:

- includes the generated Rust source from `OUT_DIR`;
- lazily deserializes the generated normalized manifest;
- retains the existing public contribution/policy helper API;
- derives the metadata property schema from the generated property editor and reuses Page Builder validation;
- contains no handwritten `ModuleContributionManifest`, `ContributionDescriptor`, `PropertyEditorDescriptor` or consumer property-field tree.

Pages remains persistence/lifecycle/publication owner. This slice changes contribution metadata authority only.

## Guard reconciliation

The Fly contribution source guard and Pages metadata-property guard now inspect the canonical module metadata plus build generator instead of requiring duplicated literals in `contributions.rs`. They also reject reintroduction of handwritten descriptor authority in the Pages runtime source.

## Phase 9 state after this slice

- [x] Separate admin/storefront factories.
- [x] Pages reference consumer generates its complete contribution manifest from canonical module metadata at build time.
- [x] Tenant, permission, capability, provider policy and provider-health filtering.
- [x] Duplicate, missing-provider, missing-dependency, cycle and provider-version diagnostics.
- [ ] Generalize the Pages build-time metadata schema/generator into shared module tooling before onboarding the second production contribution consumer.

## Validation boundary

Per maintainer instruction, no tests, Node verifiers, Cargo checks, formatting, TOML parser execution, WASM/native builds, workflows or CI were run by this slice. Source-ready does not claim executed evidence.
