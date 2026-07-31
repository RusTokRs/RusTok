# FORUM-23B2G2B3C versioned Search invalidation consumer

Status: `source_complete_runtime_evidence_pending`

## Delivered boundary

This slice adds the persistent consumer side of the Forum Search versioned
invalidation rollout without adding a second Search execution path.

The server may open one persistent sealed-contract cursor with:

- topic: `domain`;
- consumer group: `rustok-search-forum-projection-v1`;
- default-off flag: `RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED`;
- required delivery profile: `outbox_iggy`;
- required database backend: PostgreSQL.

The rollout flag is deliberately default-off. Source completion does not claim
that a production broker cursor, PostgreSQL database, restart sequence or poison
delivery has been exercised.

## One inbox and one projector

`rustok-search` remains transport-neutral. `ForumSearchContractIngress` accepts
a registered `ContractEventEnvelope`, recognizes
`forum.search_projection.invalidation_issued`, and adapts it to the existing
root-compatible Forum inbox representation.

The identities are:

```text
typed transport receipt id = ContractEventEnvelope.id
Search projection event id = ContractEventEnvelope.causation_id
legacy root envelope id     = ContractEventEnvelope.causation_id
Forum owner ledger event id = ContractEventEnvelope.causation_id
```

The typed envelope ID is never used as `search_projection_inbox.event_id`.

The adapter reuses:

- `search_projection_inbox`;
- `ForumProjectionInbox`;
- `ForumProjectionReconciler`;
- `ForumSearchProjector`;
- Search-owned `ingest_sequence` ordering.

No second inbox, reconciler, projector or projection watermark was introduced.

A typed delivery is converted to a root-compatible
`index.reindex_requested` envelope whose ID is the exact causation/root
identity. The existing inbox `ON CONFLICT (event_id) DO NOTHING` remains the
physical collapse point for legacy and typed delivery.

## Fail-closed duplicate recognition

A conflict on `event_id` is not accepted by UUID alone. After insertion or
conflict, the adapter reads the durable row and verifies:

- tenant identity;
- `source_module = forum`;
- exact scope key;
- root event type and schema version;
- root envelope ID and correlation identity;
- exact `ReindexRequested` target type and target ID;
- registered root envelope validity.

A mismatched pre-existing row is semantic poison with stable code
`forum.search_projection.contract_inbox_identity_conflict`. It cannot enter the
projector.

Missing typed causation is semantic poison with stable code
`forum.search_projection.contract_causation_required`.

## Independent clocks

`owner_revision` remains the Forum owner causal clock. It is retained for
diagnostics but is not written as Search `ingest_sequence`, is not compared
numerically with Search `ingest_sequence`, and does not replace the existing
owner-revision checkpoint protocol.

Search ordering remains the durable sequence allocated by
`search_projection_inbox`.

## Persistent cursor acknowledgement

The broker offset is committed only after one of these durable or neutral
results exists:

1. the shared inbox row was inserted;
2. an exact matching shared inbox duplicate was recognized;
3. a registered sealed event was classified as unrelated to this consumer;
4. raw decode/schema poison reached a durable poison receipt and DLQ publication,
   or a prior durable publication was recognized;
5. semantic poison reached the same durable receipt and DLQ protocol.

Transient database, receipt-store, broker publication or acknowledgement
failures do not choose a new terminal result. The exact broker offset remains
uncommitted.

If acknowledgement fails after inbox admission, redelivery recognizes the same
root row. If acknowledgement fails after poison publication, redelivery
recognizes the durable poison receipt and retries acknowledgement without
re-entering Search projection admission.

## Poison ownership

The host worker reuses the connector-owned
`iggy_connector_consumer_poison_receipts` store. Its identity binds:

- consumer group;
- stream;
- topic;
- partition;
- offset;
- exact raw broker bytes.

DLQ publication uses a deterministic broker message ID. No process-local poison
receipt or fallback is permitted.

When DLQ is disabled, the worker cannot choose a new poison terminal result and
leaves the source offset uncommitted. A previously established durable receipt
may still finish acknowledgement after DLQ is disabled.

## Runtime composition

The Iggy cursor, DLQ publication, poison receipt and source acknowledgement
remain server-owned. They are composed as a child of the existing
`forum_search_inbox_worker` service module so Forum Search background execution
has one host owner.

The Search crate has no Iggy or connector dependency and owns only the durable
inbox adaptation contract.

## Runtime evidence handoff

`FORUM-23B2G2B3D0` freezes the exact remaining evidence protocol without
fabricating execution output:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json
crates/rustok-forum/docs/forum-23b2g2b3d-runtime-evidence.md
```

The protocol requires PostgreSQL inbox admission and exact duplicate
recognition, persistent cursor restart before and after acknowledgement,
legacy-first and typed-first dual delivery, raw and semantic poison publication
and redelivery, deterministic DLQ duplicate suppression, missing-delivery owner
repair, multi-process serialization, deletion/ACL ordering, Search-disabled
behavior and `LINK-FORUM-03` correlation.

`FORUM-23B2G2B3D` remains open until those scenarios are generated and retained
from an executable runtime run on the exact reviewed source commit.

Tests, source verifiers, formatting, Cargo checks, Clippy, CI, PostgreSQL runtime
evidence and Iggy runtime evidence were not run in this slice or in D0.
