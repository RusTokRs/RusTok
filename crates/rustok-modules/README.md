# rustok-modules

## Purpose

`rustok-modules` is the mandatory Core owner of module artifact, marketplace,
installation and tenant-policy contracts. Its persistent control-plane adapters
are being moved here from the server incrementally.

## Responsibilities

- Define immutable module artifact identity, payload kind and source lineage.
- Verify digest-pinned OCI packages before installation and admit only isolated
  payload kinds to the sandbox.
- Retain and collect admitted CAS bytes through explicit owner retention
  snapshots; execution reads verified CAS only and never retries an OCI
  registry at runtime.
- Enforce descriptor-only brokered persistence; marketplace artifacts cannot
  declare SQL, native migrations, storage paths, or host handles.
- Restrict marketplace UI contributions to host-rendered declarative metadata;
  native UI packages remain static-promotion-only.
- Define the owner ports for marketplace publication, installation, activation,
  rollback and policy.
- Map installed artifacts to neutral sandbox requests and capability grants.
- Keep marketplace release, platform installation and tenant enablement separate.

## Entry points

- `ModulesModule`
- `ModuleArtifactDescriptor`
- `ArtifactReleaseDraft`
- `ArtifactRelease`
- `ModuleInstaller`
- `ArtifactRuntime`
- `OciArtifactReference`
- `OciDistributionArtifactRegistry`
- `SeaOrmArtifactInstallationStore`

## Interactions

- Resolves and verifies installed artifacts before executing them through
  `rustok-sandbox` for Rhai, WebAssembly and future sidecar execution.
- Alloy creates and evolves source-backed release drafts through these contracts.
- `apps/server` bootstraps this Core module without owning marketplace policy.

See the [local documentation](./docs/README.md).
