# Direct object-store runtime and owner-local lifecycle

- Date: 2026-07-22
- Status: Accepted

## Context

RusToK needs one object API for local development and S3-compatible production
storage. The previous `StorageBackend` and `StorageService` duplicated an
external object-store API and incorrectly made a platform helper the owner of
media, artifact, snapshot, and knowledge-source lifecycles.

Media also needs stable object placement, immutable image renditions, cleanup
evidence, and an extraction boundary that can move with its metadata database
and storage credentials.

## Decision

RusToK uses the Rust `object_store` crate directly. `rustok-storage` is a small
composition library that owns only:

- deserializable local/S3 runtime configuration;
- construction of `Arc<dyn object_store::ObjectStore>` and an optional
  `Signer`;
- public delivery-base configuration and diagnostics;
- the canonical typed object-key policy.

It does not expose a CRUD facade, own an object ledger, or own domain lifecycle
state. Each domain service calls `ObjectStore` directly and translates
`object_store::Error` at its own boundary.

Every key uses one namespace-first policy:

```text
namespace/zone/scope/identity-partition
```

Identity-addressed files use calendar partitions and a one-byte shard:

```text
media/objects/tenants/{tenant_id}/YYYY/MM/DD/{shard}/{blob_id}.{ext}
media/staging/tenants/{tenant_id}/YYYY/MM/DD/{shard}/{upload_id}.upload
```

Content-addressed objects use the same prefix policy but derive their partition
from the digest, without a date:

```text
module-artifact/objects/platform/sha256/{aa}/{bb}/{digest}
```

There is no layout version or persisted backend identifier in keys. The
current layout is authoritative. A future incompatible layout would require a
separate migration decision, not a speculative `v1` segment.

Media owns its asset, blob, rendition, upload-session, and lifecycle metadata.
In the modular monolith these tables remain Media-owned tables in the shared
PostgreSQL deployment. When Media is extracted, its schema/database and storage
credentials move with the whole module.

Original media blobs and generated renditions are immutable. Editing creates a
validated recipe and a new blob identity. A database transaction publishes the
new active reference only after the object write succeeds. Commit errors are
verified against the durable row before compensation; when the outcome is
ambiguous, the object is preserved and owner-local reconciliation resolves the
state. Failed and obsolete objects are reconciled asynchronously.

Write-port idempotency is owned by Media in durable `media_port_operations`
receipts. Receipts bind tenant, operation, request digest, status and response;
stale processing leases can be reclaimed only after the operation's owner-local
state makes retry safe, and fencing tokens prevent an old worker from completing
after its lease has been reclaimed. Remote gRPC authority is established by a
host-owned authentication/authorization interceptor, carries an explicit
operation allow-list, and is never accepted from serialized caller claims.

## Consequences

- S3 remains optional; local filesystem storage is the default development
  implementation.
- Domain modules depend on the critical `object_store` API just as they depend
  on SeaORM or Tokio; RusToK does not hide it behind forwarding methods.
- Calendar directories keep operations and manual inspection bounded without
  requiring storage listing for user-facing queries. The database remains the
  query index.
- CAS objects preserve cross-date deduplication and are never forced into the
  chronological layout.
- Cross-domain consumers use Media's typed ports and descriptors, never raw
  storage handles.
- Local filesystem production use requires an explicit durability decision;
  S3-compatible storage is the normal production target.

## Related contracts

- [`rustok-storage`](../crates/rustok-storage/README.md)
- [`rustok-media`](../crates/rustok-media/README.md)
- [Media and Search extraction boundaries](./2026-07-16-media-search-extraction-boundaries.md)
