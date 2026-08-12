# FORUM-14 Media reference admission actualization — 2026-08-12

Status: `source-ready / owner-reference-admission-ready / embedded-grpc-parity-covered / forum-persistence-open`

## Fresh cursor and roadmap audit

This slice was rechecked from `main` `389fa1acdb1bbe7f554380ecb5ea178c5f73bda9` after the Pages/Page Builder parity merge.

The canonical Forum ledger was stale in three material places:

- FORUM-14 still said `planned` even though the structural attachment-relation admission slice was already merged.
- FORUM-15 still described member-card composition/privacy/stat enrichment as unfinished even though the owner member-card service, Profiles privacy gating, bounded Forum-stat enrichment, storefront transport and storefront rendering slices are merged. Owner-safe avatar presentation plus retained browser/query-count evidence remain open.
- FORUM-34 still said `planned` even though the bounded source chain FORUM-34A through FORUM-34Q is merged. The remaining gap is the genuinely shared durable owner-data migration runner/checkpoint/replay/operator execution contract.

The canonical implementation plan is updated by this PR to record those facts without promoting any source-ready work to `done`.

## Rechecked FORUM-14 blocker

FORUM-14A deliberately stopped before persistence because Forum could only observe `MediaAssetReadPort::get_asset`. That API returns an ordinary Media DTO and does not itself publish a contract that another owner may safely treat as permission to persist a durable reference.

Media already owns the necessary lifecycle truth internally:

- asset state: `active`, `delete_pending`, `deleted`, `failed`;
- active blob identity and blob state;
- delete-request/deleted timestamps.

Forum must not copy those fields or infer lifecycle policy from Media tables.

## Media owner reference-admission capability

This slice adds a dedicated `MediaReferenceAdmissionPort` with a bounded `admit_references` operation.

The response intentionally exposes only:

- requested Media UUID;
- trusted tenant UUID;
- `referenceable: bool`.

It does **not** expose raw lifecycle strings, storage keys, deletion timestamps or a Forum-owned quarantine model.

The embedded owner implementation is referenceable only when all currently known-safe owner facts hold:

- exact tenant match;
- asset lifecycle is `active`;
- asset is not delete-requested/deleted;
- an active blob exists and matches the asset;
- blob lifecycle is `ready` and has a ready timestamp;
- blob is not delete-requested/deleted.

Missing assets and every other state return `referenceable = false`. Unknown/future states therefore fail closed automatically, including a future quarantine state if Media introduces one.

The request is bounded to 100 ids, rejects nil ids, deduplicates while preserving first-request order and returns one owner decision per normalized request.

## FBA transport parity

The capability is not embedded-only.

`rustok-media-transport` adds an explicit `AdmitReferences` gRPC operation using the existing JSON owner-contract framing. The provider-side trusted authority must separately admit `MediaGrpcOperation::AdmitReferences`; tenant/principal authority is replaced by the server-side trusted authority exactly like the other Media ports.

The loopback conformance suite exercises the same admission port through embedded `MediaService` and `GrpcMediaProvider`, including:

- active asset -> referenceable;
- missing asset -> non-referenceable;
- duplicate request ids -> one normalized result;
- missing deadline -> typed timeout policy failure;
- deleted asset -> non-referenceable without leaking raw lifecycle.

## Forum persistence gate

`ForumPreparedAttachmentRelationBatch` remains the pure structural admission result from FORUM-14A.

This slice adds `admit_attachment_relations_for_persistence`, which upgrades that value to `ForumMediaAdmittedAttachmentRelationBatch` only after one bounded Media owner call.

The Forum gate:

- preserves text-only Forum operation when the batch has no attachments;
- fails with an explicit capability-unavailable error when attachments exist but Media is not composed;
- requires Forum source tenant and Media `PortContext` tenant to match;
- deduplicates Media UUIDs before the owner call so repeated use of one asset never creates N+1 owner reads;
- requires exactly one well-formed response for every requested UUID;
- rejects foreign, duplicate, missing or extra owner results;
- rejects every `referenceable = false` asset before persistence.

The admitted wrapper has no public constructor. Future attachment persistence should require this wrapper rather than a raw/prepared UUID batch.

## Deliberately unchanged

This slice does not:

- create Forum upload/session/blob lifecycle;
- read Media private tables from Forum;
- expose Media lifecycle strings as Forum contract state;
- persist attachment rows yet;
- add attachment UI or command transport;
- claim runtime/browser evidence;
- create a Forum-local shared import runner for FORUM-34;
- mark FORUM-14, FORUM-15 or FORUM-34 done.

## Next cursor

The Media lifecycle-admission blocker for FORUM-14 persistence is now removed at the owner-contract boundary.

The next safe FORUM-14 slice is Forum-owned attachment relation persistence that accepts only `ForumMediaAdmittedAttachmentRelationBatch`, keeps source revision/order/usage/caption authoritative in Forum, and rechecks target/source-revision concurrency inside the owner transaction. After that: owner command transports, read hydration, module UI, reconciliation and retained runtime evidence.

For FORUM-15, continue with owner-safe avatar presentation through Profiles/Media public presentation contracts and retain browser/query-count evidence.

For FORUM-34, do not add a Forum-local checkpoint journal. Recheck for a genuinely shared owner-data migration runner before durable cross-batch execution.
