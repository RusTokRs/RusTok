# rustok-modules

## Purpose

`rustok-modules` is the mandatory Core owner of module artifacts, marketplace
governance, installation lifecycle and tenant module policy.

## Responsibilities

- Define immutable module artifact identity, payload kind and source lineage.
- Own marketplace publication, installation, activation, rollback and policy.
- Map installed artifacts to neutral sandbox requests and capability grants.
- Keep marketplace release, platform installation and tenant enablement separate.

## Entry points

- `ModulesModule`
- `ModuleArtifactDescriptor`
- `ArtifactReleaseDraft`
- `ArtifactRelease`

## Interactions

- Uses `rustok-sandbox` for Rhai, WebAssembly and future sidecar execution.
- Alloy creates and evolves source-backed release drafts through these contracts.
- `apps/server` bootstraps this Core module without owning marketplace policy.

See the [local documentation](./docs/README.md).

