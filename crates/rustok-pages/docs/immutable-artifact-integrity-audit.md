# Pages Immutable Artifact Integrity Audit

Date: 2026-08-06  
Status: `source-ready / maintainer-validation-pending`

## Purpose

Pages already verifies an immutable Page Builder artifact when it is staged, bound or read by the public storefront. That protects active operations, but it does not provide a bounded maintenance command for reviewing retained historical artifact rows.

This slice adds a read-only Pages-owned command:

```text
PageService::audit_immutable_artifact_integrity
```

It audits immutable artifact records for one exact tenant and page. It does not repair, rebuild, rebind, publish or delete anything.

## Authorization and scope

The command requires tenant-wide:

```text
pages:manage
```

The requested tenant and page identifiers must be non-nil. The page must exist inside the exact tenant before artifact rows are read.

The audit runs through one database transaction. PostgreSQL and MySQL use shared locks for the page and artifact rows; SQLite uses transaction serialization.

## Bounded scan

Defaults and hard limits:

```text
default records: 128
maximum records: 512
maximum returned findings: 64
```

Artifacts are ordered by:

```text
created_at ASC, id ASC
```

The query requests `max_records + 1`. When the extra row exists, the result sets:

```text
truncated = true
```

A truncated result is not a complete page audit. The caller must request another bounded operational slice or use a future cursor-based command; this source slice does not pretend otherwise.

## Integrity checks

Each retained record is reconstructed through the same public Page Builder artifact and materialization contracts used by Pages reads.

The audit checks:

- non-nil artifact identity and exact tenant/page ownership;
- non-empty locale;
- decodable build identity, registry, page head and landing section manifest;
- static artifact SHA-256 integrity;
- source/build/renderer metadata equality with the reconstructed artifact;
- document HTML, body HTML and CSS byte limits;
- legacy materialization evidence only when all three fields are `NULL`;
- current materialization evidence only when hash, identity and snapshots are all present;
- runtime snapshot and materialization SHA-256 integrity;
- stored materialization hash equality with the reconstructed identity.

Partial materialization evidence fails closed.

## Result privacy

The result includes only bounded operational metadata:

- artifact id;
- locale;
- build hash;
- optional materialization hash;
- one stable public finding code;
- SHA-256 of the internal diagnostic;
- counts, truncation flags and a deterministic audit hash.

The command does not return:

- document HTML or body HTML;
- CSS;
- page head or landing sections;
- component registry payloads;
- runtime snapshots;
- materialization identity JSON;
- internal error text;
- raw runtime context.

The audit hash binds the tenant, page, requested limit, truncation state and ordered artifact identity/status observations. It is an observation receipt, not persisted evidence and not a repair authorization.

## Preserved boundaries

This source slice does not:

- modify immutable artifacts or published bindings;
- create repair or rebuild behavior;
- emit lifecycle or cache events;
- change publication, rollback or inline-edit flows;
- add database migrations;
- add GraphQL, HTTP, OpenAPI or admin UI transport;
- change anonymous storefront reads;
- promote FFA or FBA.

Repair/rebuild remains a separate open source cursor. Public transport and executed database evidence also remain open.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit.mjs
cargo test -p rustok-pages artifact_integrity_audit -- --nocapture
cargo check -p rustok-pages
```

A retained execution packet should cover at least:

- exact `pages:manage` admission and denial;
- an empty artifact set;
- valid legacy all-`NULL` materialization evidence;
- valid current materialization evidence;
- partial evidence rejection;
- corrupted artifact/content/materialization hashes;
- output byte-limit rejection;
- 512-row completion and 513-row truncation;
- zero writes and zero emitted events.

No test, verifier, Cargo, formatting, database, workflow or CI execution is claimed here.
