# rustok-storage

## Purpose

Construct the shared direct `object_store` runtime and canonical object keys
without hiding the external library behind a RusToK CRUD facade.

## Responsibilities

- local and optional S3-compatible runtime configuration;
- `Arc<dyn object_store::ObjectStore>` and optional `Signer` composition;
- chronological and SHA-256 key construction;
- public delivery-base and backend-kind diagnostics.

## Interactions

The server creates one `StorageRuntime`. Domain owners receive it through host
composition, call `ObjectStore` directly, and own their metadata and lifecycle.
Media uses chronological tenant keys; module artifact CAS uses digest keys.

## Entry points

- `StorageRuntime`
- `StorageConfig`
- `ObjectKey::chronological`
- `DigestObjectKey::sha256`

See [module documentation](./docs/README.md) and the
[implementation plan](./docs/implementation-plan.md).
