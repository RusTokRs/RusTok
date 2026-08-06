---
id: doc://docs/modules/_template/indexing.md
kind: integration_contract
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: draft
---
# Index integration

Use this document to state whether the module participates in `rustok-index`
and which compatibility level is implemented.

## Module form

Choose exactly one:

- [ ] Native in-repository module selected into the server distribution
- [ ] Standalone `wasm32-wasip2` component

A standalone component cannot directly depend on `rustok-index`, access
PostgreSQL, register `ModuleRuntimeExtensions`, or claim that
`platform.events` publication is automatic indexing. Record the admitted host
capability or event bridge here; otherwise state that Index integration is not
available.

## Compatibility level

Choose the highest completed level:

- [ ] No Index integration
- [ ] Schema/query compatible
- [ ] Replay compatible
- [ ] Incremental compatible
- [ ] Drift-aware
- [ ] Repair-ready

## Native registration

Document:

- owner module slug;
- every versioned `SchemaRef`;
- registration entry point;
- replay source or PostgreSQL source factory;
- selected-distribution gate;
- cross-module target schemas.

Reference implementation contract:

```text
crates/rustok-index/docs/module-source-integration.md
```

## Schema contract

For each indexed entity, record:

- schema identity and version;
- locale mode;
- selectable/filterable/sortable fields;
- field types and cardinality;
- links and target schemas;
- confidentiality restrictions;
- schema migration/versioning policy.

## Replay and targeted load

Document:

- deterministic scan order;
- cursor representation and bounds;
- maximum page size;
- targeted-load key limit;
- retryable and permanent failures;
- authoritative deletion/absence behavior.

## Incremental ingestion

Document:

- stable event/delivery UUID derivation;
- monotonic source-version source;
- complete upsert payload;
- delete/tombstone retention;
- link replacement semantics;
- commit-before-ack path;
- redelivery and payload-reuse behavior.

## Drift and repair

Document only admitted behavior:

- current-version evidence;
- explicit absence watermark;
- link/target authority;
- authorization boundary;
- recovery policy;
- retained PostgreSQL evidence;
- public or automatic surfaces, when admitted.

## Prohibited shortcuts

Confirm that the module does not:

- [ ] write Index-owned tables directly;
- [ ] require Index to read source-domain tables directly;
- [ ] use unstable delivery IDs;
- [ ] use non-monotonic source versions;
- [ ] treat a missing row as proven deletion without an owner contract;
- [ ] invent a standalone `platform.index` capability;
- [ ] describe an ordinary Events message as an Index mutation without an
      admitted bridge.

## Verification

List contract tests and retained evidence for the claimed level:

- schema/fingerprint:
- scan/load bounds:
- redelivery:
- deletion:
- cross-module links:
- PostgreSQL:
- restart/crash windows:
- authorization/recovery:
