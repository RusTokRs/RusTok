# rustok-modules documentation

## Purpose

This Core module owns the module platform control plane and artifact lifecycle.

## Responsibility Zone

It owns marketplace release identity, digest-pinned package admission,
tenant-policy rules and the contracts for installation, activation, rollback,
capability grants and static-promotion admission. Persistence adapters and
owner transports are still being moved from the server. It does not own sandbox
implementation or Alloy source authoring.

## Integration

Rhai and WebAssembly artifact descriptors select executors from
`rustok-sandbox`. A package preserves both its OCI manifest digest and its
verified payload-layer digest. Alloy drafts carry source lineage and create
immutable module releases/packages. The server supplies infrastructure adapters
and mounts owner transports.

`OciDistributionArtifactRegistry` resolves only digest-pinned references. It
requires the returned manifest digest to match the requested reference, reads
the descriptor from the manifest config, and downloads exactly one payload
layer whose digest and media type match that descriptor.

During admission, `ModuleInstaller` verifies the OCI package and places its
payload in an `ArtifactBlobStore` under the descriptor payload digest.
`ArtifactRuntime` reads only that admitted digest-pinned blob for execution;
the external OCI registry is a distribution source and is not consulted at
runtime. Missing or corrupted blobs fail closed before a sandbox request is
created.

`module_artifact_installations` is the host-managed persistence boundary. Its
PostgreSQL migration enables RLS; tenant-scoped connections must set
`rustok.tenant_id` before querying or mutating tenant installation rows.
`SeaOrmArtifactInstallationStore` performs that setup in the same transaction
as its insert; it stores the reference and canonical descriptor, never artifact
bytes. A durable production blob-store adapter, atomic blob/installation
publication, and retention/garbage collection remain the next CAS work slice.

## Verification

- `cargo xtask module validate modules`
- `cargo test -p rustok-modules`
- `cargo check -p rustok-server --lib`

## Related Documents

- [Implementation plan](./implementation-plan.md)
- [Neutral sandbox ADR](../../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md)
- [Module control-plane plan](../../../docs/modules/module-control-plane-consolidation-plan.md)
