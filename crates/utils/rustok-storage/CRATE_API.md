# rustok-storage — Crate API

Runtime construction and canonical object-key policy for RusToK.

## Public surface

- Object keys: `ObjectKey`, `DigestObjectKey`, `ObjectScope`, `ObjectZone`, `KeyError`.
- Runtime: `StorageRuntime`, `StorageConfig`, `StorageKind`, `StorageDriver`, `LocalStorageConfig`, `S3StorageConfig`.
- `object_store` — the direct external crate, re-exported without wrapping.

## Invariants

- Domain owners use `object_store::ObjectStore` directly; this crate does not wrap object CRUD or own any domain object's lifecycle.
- Object keys are canonical: every participant derives keys through the same policy so no ad-hoc key formats appear.
- Zone and scope selection is explicit at key construction; keys never encode tenant-free or scope-free paths.
