# Pages Immutable Artifact Integrity Audit

Date: 2026-08-07  
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

Only `PermissionScope::All` is accepted by the owner service. Under the current access-token permission bridge, effective `pages:manage` resolves to `PermissionScope::All` when present and `PermissionScope::None` when absent; there is no current `PermissionScope::Own` outcome for Pages Manage. The explicit owner `All` check remains defense in depth for direct/internal callers and future authorization-model changes.

The requested tenant and page identifiers must be non-nil. The page must exist inside the exact tenant before artifact rows are read.

The audit runs through one database transaction. PostgreSQL and MySQL use shared locks for the page and artifact rows; SQLite uses transaction serialization. This is a single-transaction read boundary, not a claim that every supported backend provides the same repeatable-read isolation semantics.

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

The first query projects artifact identifiers only and requests `max_records + 1`. When the extra identifier exists, the result sets:

```text
truncated = true
```

Each selected full artifact payload is then loaded and verified one record at a time under the same transaction. The command never retains hundreds of HTML/CSS/runtime payloads in one in-memory result set.

A truncated result is not a complete page audit. The caller must request another bounded operational slice or use a future cursor-based command; this source slice does not pretend otherwise.

## Integrity checks

Each retained record is reconstructed through the same public Page Builder artifact and materialization contracts used by Pages reads.

The audit checks:

- non-nil artifact identity and exact tenant/page ownership;
- non-empty locale;
- decodable build identity, registry, page head and landing section manifest;
- static artifact SHA-256 integrity;
- source/build/renderer metadata equality with the reconstructed artifact;
- document HTML, body HTML and CSS byte limits before payload cloning;
- legacy materialization evidence only when all three fields are `NULL`;
- current materialization evidence only when hash, identity and snapshots are all present;
- runtime snapshot and materialization SHA-256 integrity;
- stored materialization hash equality with the reconstructed identity.

Partial materialization evidence fails closed. JSON evidence is deserialized from borrowed values, avoiding a second full `serde_json::Value` copy.

## Result privacy

The result includes only bounded operational metadata:

- artifact id;
- SHA-256 locale hash;
- SHA-256 record-identity hash binding owner and retained artifact hash fields;
- one stable public finding code;
- SHA-256 of the internal diagnostic;
- counts, truncation flags and a deterministic audit hash.

The command does not return:

- raw locale;
- raw build, artifact, content or materialization hashes;
- document HTML or body HTML;
- CSS;
- page head or landing sections;
- component registry payloads;
- runtime snapshots;
- materialization identity JSON;
- internal error text;
- raw runtime context.

Hashing record identity instead of returning stored text keeps every finding byte-bounded even when a corrupted database row contains an unexpectedly large text value. Record and final audit identities are serialized directly into a streaming SHA-256 writer rather than an intermediate JSON byte vector.

The audit hash binds the tenant, page, requested limit, truncation state and ordered hashed artifact identity/status observations. It is an observation receipt, not persisted evidence and not a repair authorization.

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

Repair/rebuild remains a separate source cursor. Public transport and executed database evidence also remain open.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit.mjs
cargo test -p rustok-pages artifact_integrity_audit -- --nocapture
cargo check -p rustok-pages
```

A retained execution packet should cover at least:

- exact current-tenant effective `pages:manage` admission and `PermissionScope::All` owner check;
- missing-permission denial and the current `All`/`None` scope semantics;
- an empty artifact set;
- valid legacy all-`NULL` materialization evidence;
- valid current materialization evidence;
- partial evidence rejection;
- corrupted artifact/content/materialization hashes;
- oversized corrupted identity text with bounded result fields;
- output byte-limit rejection before cloning;
- 512-row completion and 513-row truncation;
- one-at-a-time full payload loading;
- zero writes and zero emitted events.

No test, verifier, Cargo, formatting, database, workflow or CI execution is claimed here.
