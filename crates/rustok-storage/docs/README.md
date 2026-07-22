# `rustok-storage` documentation

`rustok-storage` constructs RusToK's direct `object_store` runtime and owns the
canonical object-key policy. It is infrastructure support, not a domain service
and not the owner of stored objects.

## Responsibilities

- configure local and optional S3-compatible `object_store` implementations;
- expose the direct object handle and optional native signer;
- construct safe chronological and digest-addressed keys;
- expose delivery-base and backend-kind diagnostics;
- document backend durability, concurrency, and operational requirements.

## Non-responsibilities

- no custom storage trait or CRUD facade;
- no global object ledger;
- no media, module, AI, snapshot, or knowledge-source lifecycle state;
- no user-facing directory browser;
- no persisted driver name or layout version.

Owner modules call `ObjectStore` directly and keep their own metadata. Media
queries its database, not object-store listings. Trusted reconciliation and CAS
jobs may list only their controlled namespace prefix.

## Production operations and recovery

- Readiness performs a bounded write/delete probe under
  `platform-health/staging/platform/...` and exports backend kind plus health;
  it never creates domain rows.
- Alert on a failed storage readiness check, Media reconciliation `retry_later`,
  growing `delete_pending` blobs, or staging sessions that remain uncleaned.
- Treat the database as the authoritative index. Never repair an incident by
  deleting rows or scanning all buckets into a new media catalog.
- Run `rustok-cli media reconcile --limit <count>` repeatedly after transient
  storage recovery. Missing objects become explicit failed evidence; pending
  deletes and expired staging objects are retried safely.
- Restore database and object storage from the same recovery point. If that is
  impossible, restore objects first, then run reconciliation and review failed
  assets before reopening writes.
- For Local storage, coordinate filesystem snapshots with database snapshots and
  choose `fsync=true` when the durability requirement outweighs throughput.
- For S3-compatible storage, configure provider versioning/retention as required
  and a lifecycle rule that aborts incomplete multipart uploads. RusToK still
  calls multipart abort on controlled failure paths.
- Rotate S3 credentials through host configuration. Presigned URLs are short
  lived and scoped to one exact object key; they are not credentials to persist.

The Local/S3 conformance suite is the pre-deployment backend gate. Run it
against the exact provider endpoint and bucket used by the environment.

## Related documents

- [Implementation plan](./implementation-plan.md)
- [Direct object-store ADR](../../../DECISIONS/2026-07-22-direct-object-store-runtime-owner-local-lifecycle.md)
- [`rustok-media` documentation](../../rustok-media/docs/README.md)
