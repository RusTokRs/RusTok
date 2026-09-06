# FORUM-23B2G2B3D10 normal delivery proof

## Status

`source_ready_maintainer_execution_pending`

This slice supplies the direct `normal_delivery` trace required by the frozen
`FORUM-23B2G2B3D0` runtime-evidence matrix. Earlier subproofs independently
covered PostgreSQL ingress, Iggy acknowledgement/restart, owner-ledger repair,
projection ordering and storefront visibility. D10 correlates the ordinary
success path through one shared set of Forum owner, broker, Search inbox and
checkpoint identities.

The machine-readable proof contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-normal-delivery-proof.json
```

The executable test is:

```text
apps/server/tests/forum_versioned_invalidation_normal_delivery_iggy.rs
```

Successful execution writes:

```text
target/forum-search-versioned-invalidation-normal-delivery-evidence.json
```

The artifact is written only after the complete scenario and isolated
PostgreSQL schema cleanup succeed. It records the exact Git source commit and
must not be hand-edited.

## One correlated owner trace

The test applies real Outbox, Taxonomy, Forum and Search migrations in one
isolated PostgreSQL schema. Real Forum services create:

1. one public category;
2. one public topic containing `d10normaldeliverytopic`.

Those two owner transactions must commit this exact ledger shape:

```text
revision 1: forum          / null
revision 2: forum_category / category ID
```

For each revision, the test reads the committed legacy root and typed contract
envelopes from the transactional outbox and requires:

```text
forum_projection_revision_ledger.event_id
  = legacy EventEnvelope.id
  = typed ContractEventEnvelope.causation_id
```

The typed envelope ID must differ from the root ID and remains transport
identity only.

## External Iggy delivery

The exact typed envelopes committed by Forum are published to a unique external
Iggy stream in owner-revision order. The production persistent group
`rustok-search-forum-projection-v1` receives both messages from topic `domain`.

For each broker delivery, the test requires:

- the exact typed envelope bytes decode successfully;
- typed ID, tenant ID and causation root match the owner transaction;
- a broker offset and acknowledgement token are present;
- production `ForumSearchContractIngress` creates one pending Search inbox row
  keyed by the root ID;
- source acknowledgement happens only after durable ingress succeeds.

Broker offsets and Search `ingest_sequence` values must each increase, but they
remain independent clocks and are never compared to Forum owner revisions.

## Projection and delivery-covered checkpoint

After both source acknowledgements, the production Forum projection source and
`ForumProjectionReconciler::with_owner_revision_source` process the two pending
inbox rows. The expected report is:

```text
due tenants:                  1
claimed inbox rows:           2
completed inbox rows:         2
failed inbox rows:            0
owner tenants reconciled:     1
owner revisions checkpointed: 2
owner repair rebuilds:        0
```

Revision coverage comes from the completed rows carrying the exact ledger root
IDs. No missing-delivery repair rebuild is permitted on the ordinary path.

A PostgreSQL audit trigger captures every owner checkpoint mutation. It must
observe:

```text
revision 1 / delivery_covered / 2 Forum documents
revision 2 / delivery_covered / 2 Forum documents
```

The final checkpoint references the revision-2 root UUID. Projection therefore
commits before either checkpoint mutation.

## Storefront assertion

The retained Search projection must contain exactly the current category and
topic. The same production anonymous Forum storefront Search execution and
current owner eligibility boundary then query `d10normaldeliverytopic` and must
return exactly the created topic with `total = 1`.

A caught-up repeat sweep must claim no inbox row, perform no owner rebuild and
advance no checkpoint.

## Deliberate boundary

D10 proves the successful external-broker delivery path. It does not repeat the
failure/restart, poison/DLQ, multi-process, deletion/ACL or Search-disabled
scenarios owned by D3-D9. It does not assemble the aggregate D0 artifact or
close `FORUM-23` or `LINK-FORUM-03`.

No production Rust path, dependency, migration, event schema, digest, public
DTO, runtime flag, `Cargo.toml` or `Cargo.lock` entry changes in this slice.

## Maintainer verification

```bash
node scripts/verify/verify-forum-search-versioned-invalidation-normal-delivery-proof.mjs
RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" \
RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS="127.0.0.1:8090" \
  cargo test -p rustok-server \
  --test forum_versioned_invalidation_normal_delivery_iggy \
  -- --nocapture --test-threads=1
```

No command above was run by the implementation agent.
