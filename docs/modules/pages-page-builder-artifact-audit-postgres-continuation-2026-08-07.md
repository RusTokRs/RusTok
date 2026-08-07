# Pages / Page Builder Immutable Artifact Audit PostgreSQL Continuation

Date: 2026-08-07  
Status: source-ready / immutable-artifact-audit-postgres-harness-source-ready / execution-pending  
Scope: PostgreSQL shared-lock scan source evidence for the bounded immutable artifact integrity audit owner

## Rechecked state

The audit owner, bounded GraphQL/HTTP transports and SQLite owner harness are already merged. The remaining audit-specific backend gap was PostgreSQL: the production owner takes shared locks for its page fence, bounded artifact-id projection and each selected artifact record, while the SQLite packet cannot exercise those PostgreSQL locking primitives.

This packet adds environment-gated PostgreSQL source evidence without changing production behavior.

Source marker:

```text
immutable-artifact-audit-postgres-harness-source-ready
```

Harness:

```text
crates/rustok-pages/tests/immutable_artifact_integrity_audit_postgres.rs
```

Machine evidence:

```text
crates/rustok-pages/contracts/evidence/pages-immutable-artifact-integrity-audit-postgres-source.json
```

Fail-closed source guard:

```text
crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit-postgres.mjs
```

## PostgreSQL fixture

The harness is gated by `RUSTOK_PAGES_TEST_DATABASE_URL` with `DATABASE_URL` fallback and accepts only `postgres://` or `postgresql://` URLs. It creates a unique schema, applies the real `OutboxModule` and `PagesModule` migrations, installs only the tenant-module enablement fixture required by Pages, and drops the schema after the scenario.

Reviewed publish follows the current production `reviewed_publish::body_revision_snapshot` contract exactly:

```text
body.updated_at.to_string()
```

The harness therefore sends the created body DTO's matching `body.updated_at` value directly. It does not append a content digest. The source guard binds this fixture to the production owner and forbids the stale `updated_at:sha256(format\0content)` construction in this audit packet.

No custom artifact/audit tables are created.

## Valid canonical and rebuilt scan

A real reviewed publish creates the canonical immutable artifact and retained rebuild provenance. A real explicit rebuild appends the operation-bound immutable copy.

The owner audit with `max_records=2` must retain:

```text
scanned = 2
valid   = 2
invalid = 0
truncated = false
```

The same two-record dataset is audited with `max_records=1`. This source packet therefore also retains the owner bounded-projection branch:

```text
scanned = 1
truncated = true
```

This matches the production `fetch_limit = max_records + 1` contract without requiring an artificial large fixture.

## Shared-lock packet

The production PostgreSQL owner has three lock-backed stages inside one transaction:

1. tenant/page fence: shared lock on the page row;
2. bounded projection: shared lock while selecting artifact IDs in stable `created_at, id` order;
3. materialization: shared lock while reading each selected full artifact record.

The harness reproduces those exact SeaORM `lock_shared()` primitives against the same Pages entities. While those locks are held, a second PostgreSQL transaction sets:

```text
SET LOCAL lock_timeout = '100ms'
```

and attempts to update the locked immutable artifact payload. The source assertion requires that write to fail specifically with a `lock timeout`, then rolls the updater transaction back, commits the shared-lock transaction and verifies that the artifact document is unchanged.

This is intentionally split into two evidence layers:

- the runtime harness covers PostgreSQL shared-lock conflict behavior on the same entity/query shapes;
- the fail-closed source guard binds the production audit owner to its three PostgreSQL `lock_shared()` branches.

Until the maintainer runs the packet, neither layer is claimed as executed evidence.

## Corrupt payload and partial materialization

The PostgreSQL owner packet also retains the same bounded semantics already source-covered in SQLite.

For a deliberately changed immutable `document_html`, the audit command remains successful but returns one invalid finding with:

- artifact UUID;
- static `PAGE_ARTIFACT_INTEGRITY_INVALID` code;
- SHA-256 locale hash;
- SHA-256 record-identity hash;
- SHA-256 diagnostic hash.

For an otherwise current materialized artifact with only `runtime_snapshots` removed, the audit must classify the record invalid through the partial materialization branch and return the same hashed finding shape.

Raw locale text, HTML/CSS, stored hashes, runtime snapshots and internal diagnostic text are not part of the bounded result contract.

## Preserved boundaries

- No production service/entity/cache/adapter source is changed.
- No migration or database schema is changed.
- No GraphQL/HTTP/OpenAPI surface is changed.
- Audit remains read-only and never repairs, rebuilds, activates, publishes or invalidates cache automatically.
- The direct artifact updates in the harness are corruption/concurrency fixtures only.
- No automatic audit-to-rebuild or rebuild-to-activation behavior is introduced.
- No FFA/FBA promotion is made.

## Evidence state

Status remains:

```text
pages_immutable_artifact_integrity_audit_postgres_source_unvalidated
```

`execution` is empty and every validation flag remains false. The source guard and PostgreSQL harness are intentionally not run in this slice.

## Updated broader cursor

| Capability | Source state | Execution state |
| --- | --- | --- |
| Immutable artifact audit owner | Source-ready | Runtime evidence pending |
| Immutable artifact audit GraphQL/HTTP transports | Source-ready | Transport execution pending |
| Audit reviewed-publish revision fixture | Owner-aligned | Execution pending |
| Audit Manage `All`/`None` authorization | SQLite harness-ready | SQLite execution pending |
| Audit canonical and rebuilt artifact semantics | SQLite + PostgreSQL harness-ready | Execution pending |
| Audit bounded truncation (`max_records=1`) | SQLite + PostgreSQL harness-ready | Execution pending |
| Audit corrupted immutable payload finding | SQLite + PostgreSQL harness-ready | Execution pending |
| Audit partial materialization finding | SQLite + PostgreSQL harness-ready | Execution pending |
| Audit PostgreSQL page/id/record shared-lock source binding | Harness + guard ready | PostgreSQL execution pending |
| Audit PostgreSQL concurrent update `lock_timeout` | Harness-ready | PostgreSQL execution pending |
| Provenance migration/publish rollback/loss evidence | Source owner exists | Dedicated packet still open |
| Explicit repair owner/transport/cache packets | Source-ready | Maintainer execution pending; revision-fixture recheck remains separate |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Align the remaining repair PostgreSQL/negative/cache publish-revision fixtures with the current reviewed-publish owner before their maintainer execution.
2. Execute the SQLite and PostgreSQL audit harnesses plus both audit source guards and retain accepted evidence.
3. Retain the dedicated provenance migration/publish packet for exact locale capture, aggregate-hash mismatch rollback, artifact-row loss and legacy no-backfill behavior.
4. Execute the repair transport/request/PostgreSQL/failure/cache packets after their revision-fixture cleanup.
5. Retain successful bounded audit transport execution with current-tenant and Pages Manage fencing.
6. Execute the broader artifact/HTTP/browser and tenant Wave packets before any FFA/FBA promotion.
7. Keep automatic audit-to-rebuild and rebuild-to-activation chaining absent until accepted execution evidence supports any policy change.

## Maintainer validation

Suggested commands, intentionally not run here:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit-postgres.mjs
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test immutable_artifact_integrity_audit_postgres -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit-sqlite.mjs
cargo test -p rustok-pages --test immutable_artifact_integrity_audit_sqlite -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit.mjs
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo commands, formatting, PostgreSQL/SQLite scenarios, GraphQL/HTTP requests, workflows and CI were intentionally not run.
