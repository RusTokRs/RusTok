# `rustok-storage` Documentation

`rustok-storage` — shared storage abstraction layer of the platform. It provides a unified
`StorageBackend` contract for domain modules that need to store files,
regardless of the specific backend.

## Purpose

- publish the canonical storage backend contract;
- isolate domain modules from the details of local/S3-compatible storage implementation;
- maintain a unified high-level `StorageService` for file-oriented platform scenarios.

## Scope

- `StorageBackend`, `UploadedObject`, `StorageService`;
- conditional object creation and trusted-prefix listing for durable
  content-addressed storage reconciliation;
- backend selection/configuration and path generation helpers;
- local storage implementation and future backend seams;
- storage errors, public URL construction and path-safety guarantees;
- absence of domain-owned media/business logic.

## Integration

- used by `rustok-media` and other file-oriented modules as a shared storage dependency;
- `rustok-modules` uses the same contract for `StorageArtifactBlobStore`; its
  production configuration must select durable object storage (for example
  S3-compatible storage), never a node-local cache;
- `apps/server` acts only as a wiring layer for registering `StorageService`;
- storage health and basic observability must remain synchronized with host/runtime docs;
- domain modules must not bypass `rustok-storage` with direct backend-specific code without a clear reason.

## Verification

- structural verification: local docs and the storage contract must remain synchronized;
- targeted compile/tests when changing `StorageBackend`, path safety or backend configuration;
- integration checks needed when changing backend implementations and health semantics.

## Content-addressed storage guarantees

`store_if_absent` provides conditional final-object publication. Callers must
derive final paths from verified content digests and re-read an already-present
object before accepting it. `list` is restricted to a trusted internal prefix
and exists for reconciler jobs; it is not a user-facing file browser. The local
backend is suitable for development only. Production CAS must use the durable
object-storage driver and private staging/final prefixes.

## Related documents

- [Implementation plan](./implementation-plan.md)
- [`rustok-media` documentation](../../rustok-media/docs/README.md)
- [Observability quickstart](../../../docs/guides/observability-quickstart.md)
