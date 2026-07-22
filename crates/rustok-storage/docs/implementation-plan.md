# Implementation plan for `rustok-storage`

## Target state

`rustok-storage` is infrastructure support, not a storage domain. It constructs
the direct `object_store` runtime for Local or optional S3-compatible storage,
publishes the canonical key policy, and provides backend-compatible write
metadata and signing. It owns no CRUD facade, object ledger, or domain lifecycle.

## Status

- Status: `complete`
- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`

This crate has no UI or cross-module business port.

## Runtime contract

- `StorageRuntime.objects`: `Arc<dyn object_store::ObjectStore>` used directly.
- `StorageRuntime.signer`: optional native GET/PUT presigner.
- `StorageRuntime.kind`: diagnostics only; never persisted as domain state.
- `StorageConfig`: Local by default; S3 support is a Cargo feature.
- Local mode cleans empty directories and exposes explicit `fsync` policy.
- `put_options` emits no attributes for Local and `Content-Type` for S3.
- Credentials come from host configuration and never enter keys, rows, traces,
  metrics, or logs.

## Key contract

All keys are relative `object_store::path::Path` values:

```text
namespace/{objects|staging}/{tenants/{id}|platform}/YYYY/MM/DD/{shard}/{id}.{ext}
namespace/objects/{tenants/{id}|platform}/sha256/{aa}/{bb}/{digest}
```

Identity and time are supplied by the owner. Namespaces/extensions are
controlled lowercase ASCII. No mutable filename, title, locale, backend name,
or layout version enters a key. Database indexes, not storage folders, serve
domain queries.

## Completed delivery

1. Replaced custom backend/service types with `StorageRuntime` and typed keys.
2. Cut every owner over to direct `ObjectStore` operations and removed the
   forwarding facade.
3. Migrated Media, registry artifacts, module artifact CAS/data, and snapshot
   owners to chronological or digest keys. Durable staging keys also use the
   chronological policy.
4. Added Local and env-gated S3-compatible conformance for conditional create,
   read, prefix listing, multipart abort, delete, and GET/PUT signing.
5. Added live Media Local/S3 lifecycle evidence and production health/recovery
   guidance.

## Verification

- `cargo test -p rustok-storage --all-features`
- `cargo check -p rustok-storage --all-features`
- `cargo test -p rustok-media`
- `cargo test -p rustok-media --features s3 --test s3_lifecycle`
- repository guard for removed `StorageService`/`StorageBackend` APIs;
- repository guard for owner-local ad hoc durable key generation.

The S3 suites activate with `RUSTOK_TEST_S3_ENDPOINT`,
`RUSTOK_TEST_S3_BUCKET`, `RUSTOK_TEST_S3_ACCESS_KEY`, and
`RUSTOK_TEST_S3_SECRET_KEY`.

## Change rules

1. Do not add CRUD forwarding methods or domain lifecycle metadata here.
2. Evolve the single key policy through typed constructors and an ADR.
3. Keep runtime configuration, host health behavior, and owner runbooks in sync.
