# Pages / Page Builder Immutable Artifact Audit SQLite Continuation

Date: 2026-08-07  
Status: source-ready / immutable-artifact-audit-sqlite-harness-source-ready / execution-pending  
Scope: owner-level SQLite evidence for bounded immutable artifact integrity audit behavior

## Rechecked state

The immutable artifact audit owner and bounded GraphQL/HTTP transports are already merged. The repair programme now also has source-ready request, PostgreSQL atomicity, negative repair and after-commit cache packets. What was still missing from the broader parity cursor was a runnable database harness for the audit owner itself.

This continuation adds that missing SQLite source packet without changing production behavior.

A later source packet now also covers the PostgreSQL-specific lock-backed audit cursor:

```text
docs/modules/pages-page-builder-artifact-audit-postgres-continuation-2026-08-07.md
```

Both SQLite and PostgreSQL audit evidence remain execution-pending.

Source marker:

```text
immutable-artifact-audit-sqlite-harness-source-ready
```

Harness:

```text
crates/rustok-pages/tests/immutable_artifact_integrity_audit_sqlite.rs
```

Machine evidence:

```text
crates/rustok-pages/contracts/evidence/pages-immutable-artifact-integrity-audit-sqlite-source.json
```

Fail-closed source guard:

```text
crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit-sqlite.mjs
```

## Reviewed publish revision contract

The audit fixture must follow the current production `reviewed_publish::body_revision_snapshot` exactly. The owner currently snapshots each body revision as:

```text
body.updated_at.to_string()
```

The harness therefore supplies the created body DTO's matching `body.updated_at` value directly. It does **not** append a content digest. The source guard binds the fixture to the owner implementation and forbids the stale `updated_at:sha256(format\0content)` construction in this audit packet.

This correction changes test scaffolding only; production reviewed-publish behavior is unchanged.

## Covered owner scenarios

The harness uses isolated in-memory SQLite databases plus the real Outbox system-event migration, Channel migrations and Pages migrations. It reviewed-publishes a real GrapesJS page through `PageService` and uses the same retained provenance/rebuild owner path already used by the explicit repair packets.

### Authorization

The current Pages Manage model is retained explicitly:

```text
pages:manage present  -> PermissionScope::All
pages:manage absent   -> PermissionScope::None
```

`SecurityContext::system()` is asserted as `All`; `SecurityContext::public_read()` is asserted as `None`. The no-Manage request is rejected by `audit_immutable_artifact_integrity` before page/artifact reads.

### Valid canonical and rebuilt artifacts

The harness reviewed-publishes one canonical artifact and then appends one explicit rebuilt artifact from retained provenance. A bounded owner audit with `max_records=2` must report:

```text
scanned = 2
valid   = 2
invalid = 0
truncated = false
```

This binds both accepted instance identities to the actual owner audit:

```text
canonical
rebuild:<operation-id>
```

### Bounded truncation

The same two-artifact dataset is audited with `max_records=1`. Because the owner fetches at most `max_records + 1`, this small fixture is sufficient to prove the bounded truncation branch:

```text
scanned = 1
truncated = true
```

No artificial hundreds-of-row fixture is needed to retain this contract.

### Corruption

The canonical artifact `document_html` is deliberately changed without updating its immutable integrity identities. The audit must keep the command successful but classify the record invalid and expose only the bounded finding contract:

- artifact UUID;
- fixed `PAGE_ARTIFACT_INTEGRITY_INVALID` code;
- SHA-256 locale hash;
- SHA-256 record-identity hash;
- SHA-256 diagnostic hash.

The raw locale, HTML, CSS, stored hashes, runtime snapshots and internal error text are not asserted as public output.

### Partial materialization

A current materialized artifact initially has all three materialization fields populated. The harness removes only `runtime_snapshots`, retaining a partial tuple. The audit must classify the record invalid through the existing `Stored landing materialization evidence is partial` owner branch and return the same hashed finding shape.

## Preserved boundaries

- No production service/entity/cache/adapter source is changed.
- No migration or database schema is changed.
- No GraphQL/HTTP/OpenAPI surface is changed.
- Audit remains read-only; it does not repair, rebuild, activate, publish, rollback or invalidate caches.
- No automatic audit-to-rebuild or rebuild-to-activation behavior is introduced.
- No FFA/FBA promotion is made.

## Evidence state

Status remains:

```text
pages_immutable_artifact_integrity_audit_sqlite_source_unvalidated
```

`execution` is empty and every validation flag is false. The source guard and SQLite harness are intentionally not run in this slice.

## Updated broader cursor

| Capability | Source state | Execution state |
| --- | --- | --- |
| Immutable artifact audit owner | Source-ready | Runtime evidence pending |
| Immutable artifact audit GraphQL/HTTP transports | Source-ready | Transport execution pending |
| Audit reviewed-publish revision fixture | Owner-aligned | Execution pending |
| Audit Manage `All`/`None` owner authorization | Harness-ready | SQLite execution pending |
| Audit valid canonical/rebuilt records | Harness-ready | SQLite execution pending |
| Audit bounded record truncation (`max_records=1`) | Harness-ready | SQLite execution pending |
| Audit corrupted immutable payload finding | Harness-ready | SQLite execution pending |
| Audit partial materialization finding | Harness-ready | SQLite execution pending |
| Audit PostgreSQL locking/scan evidence | Harness + guard ready | PostgreSQL execution pending |
| Provenance migration/publish rollback/loss evidence | Source owner exists | Dedicated source packet still open |
| Explicit repair owner/transport/cache packets | Source-ready | Maintainer execution pending; revision-fixture recheck remains separate |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Align the remaining repair PostgreSQL/negative/cache publish-revision fixtures with the current reviewed-publish owner before their maintainer execution.
2. Execute the SQLite and PostgreSQL audit harnesses plus their source guards together with the existing audit owner/transport guard; retain accepted evidence.
3. Retain provenance migration/publish evidence for exact locale capture, aggregate-hash mismatch rollback, artifact-row loss and legacy no-backfill behavior.
4. Execute the repair transport/request/PostgreSQL/failure/cache packets after their revision-fixture cleanup.
5. Execute artifact/HTTP/browser and tenant Wave packets before FFA/FBA promotion.
6. Keep automatic audit-to-rebuild and rebuild-to-activation chaining absent until accepted execution evidence supports any policy change.

## Maintainer validation

Suggested commands, intentionally not run here:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit-sqlite.mjs
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit.mjs
cargo test -p rustok-pages --test immutable_artifact_integrity_audit_sqlite -- --nocapture
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test immutable_artifact_integrity_audit_postgres -- --nocapture
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo commands, formatting, SQLite/PostgreSQL scenarios, GraphQL/HTTP requests, workflows and CI were intentionally not run.
