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
- backend selection/configuration and path generation helpers;
- local storage implementation and future backend seams;
- storage errors, public URL construction and path-safety guarantees;
- absence of domain-owned media/business logic.

## Integration

- used by `rustok-media` and other file-oriented modules as a shared storage dependency;
- `apps/server` acts only as a wiring layer for registering `StorageService`;
- storage health and basic observability must remain synchronized with host/runtime docs;
- domain modules must not bypass `rustok-storage` with direct backend-specific code without a clear reason.

## Verification

- structural verification: local docs and the storage contract must remain synchronized;
- targeted compile/tests when changing `StorageBackend`, path safety or backend configuration;
- integration checks needed when changing backend implementations and health semantics.

## Related documents

- [Implementation plan](./implementation-plan.md)
- [`rustok-media` documentation](../../rustok-media/docs/README.md)
- [Observability quickstart](../../../docs/guides/observability-quickstart.md)
