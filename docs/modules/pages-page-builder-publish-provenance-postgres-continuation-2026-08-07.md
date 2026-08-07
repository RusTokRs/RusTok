# Pages / Page Builder Publish Rebuild Provenance PostgreSQL Continuation

Date: 2026-08-07  
Status: source-ready / publish-rebuild-provenance-postgres-harness-source-ready / execution-pending  
Scope: reviewed-publish rebuild provenance exact capture, aggregate rollback and artifact-loss retention

## Rechecked source state

Current Pages source already retains one `page_publish_rebuild_sources` row per published locale from the publish-operation `after_save` hook. The source row is written only after current immutable bindings and current sanitized Page Builder source snapshots reproduce the aggregate hashes stored on the publish receipt.

This packet adds the dedicated execution scaffolding that was still open in the broader parity cursor. It does not change production behavior.

Source marker:

```text
publish-rebuild-provenance-postgres-harness-source-ready
```

Harness:

```text
crates/rustok-pages/tests/publish_rebuild_provenance_postgres.rs
```

Machine evidence:

```text
crates/rustok-pages/contracts/evidence/pages-publish-rebuild-provenance-postgres-source.json
```

Fail-closed source guard:

```text
crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance-postgres.mjs
```

## PostgreSQL fixture

The harness is gated by `RUSTOK_PAGES_TEST_DATABASE_URL` with `DATABASE_URL` fallback and accepts PostgreSQL URLs only. It creates one isolated schema, applies the real `OutboxModule` and `PagesModule` migrations, enables the Pages module for one tenant and drops the schema after the scenario.

No production-owned artifact, publish, manifest or provenance table is recreated by the fixture.

## Exact two-locale source capture

The harness creates one draft with `en` and `fr` translations. The English Page Builder body is created through `PageService::create`; the French body is added through the real `PageService::save_document` path using the exact initial document fence:

```text
page:<page-id>:initial
```

Reviewed publish then supplies the current owner revision shape for both locales:

```text
(locale, body.updated_at.to_string())
```

The request intentionally provides the locale revisions in non-sorted order. The production reviewed-publish owner normalizes them before comparing with its locale-ordered current body snapshot.

After publication the packet requires exactly two locale-ordered rebuild source rows:

```text
en
fr
```

Each source row is bound back to the actual stored body and immutable artifact. The retained contract covers:

- operation, tenant and page identity;
- exact locale and body id;
- `grapesjs` source format;
- exact source revision equal to the body's `updated_at` value;
- reviewed runtime hash;
- sanitized/source/artifact/materialization SHA-256 identities;
- materialization identity and runtime snapshots copied from the exact immutable artifact;
- one retained provenance hash.

## Aggregate-hash mismatch rollback

The production `page_publish_operation::ActiveModelBehavior::after_save` invokes the publish-manifest/provenance owner for every inserted publish receipt.

The harness reuses the successfully published page and current immutable bindings, then inserts two test-owned publish receipt candidates inside explicit PostgreSQL transactions:

1. one with a different valid-shaped `artifact_set_hash`;
2. one with a different valid-shaped `sanitized_set_hash`.

Both inserts go through the real publish-operation `after_save` hook.

The owner recomputes the current locale-ordered aggregates before inserting any manifest or provenance row. Therefore each mismatch must reject the receipt persistence, after which the harness rolls the surrounding transaction back and requires:

- no fake publish operation row;
- no fake `page_publish_operation_artifacts` rows;
- no fake `page_publish_rebuild_sources` rows;
- unchanged total publish-operation, manifest and provenance counts for the page.

This retains the transaction boundary without changing production code or creating a second provenance writer.

## Artifact-row loss survivability

Migration `m20260806_000013_create_page_publish_rebuild_sources` deliberately stores `artifact_id` as retained evidence but does not create a foreign key from the provenance row to `page_static_landing_artifacts`.

The runtime harness demonstrates the resulting retention boundary using the French source row. It first removes the current binding and publish-manifest rows that independently reference the selected immutable artifact. It then deletes that exact artifact row through the real entity table.

The selected `page_publish_rebuild_sources` row must remain readable and byte-for-byte equal to its pre-loss model, including the now-missing artifact id and retained artifact/materialization identities.

The harness does not call rebuild automatically after loss. Artifact loss remains investigation input until an explicit tenant-admin repair request is made.

## Legacy no-backfill migration boundary

The provenance migration is intentionally forward-only. It creates the new table and indexes and has exactly one foreign key, to `page_publish_operations`.

The source guard binds these migration facts and fails closed if the migration gains:

- an artifact foreign key;
- a second foreign key;
- an `INSERT INTO page_publish_rebuild_sources` or equivalent query-builder insert/backfill path;
- an entity read intended to enumerate legacy publish receipts.

Therefore existing publish operations remain deliberately unbackfilled at source level. This packet does not claim a historical-database migration run until maintainer execution provides one.

## Preserved boundaries

- No production service/entity/hook code is changed.
- No migration or database schema is changed.
- No GraphQL/HTTP/OpenAPI surface is changed.
- No automatic audit-to-rebuild or publish-to-rebuild behavior is introduced.
- No automatic rebuild-to-activation behavior is introduced.
- No FFA/FBA promotion is made.

## Evidence state

Status remains:

```text
pages_publish_rebuild_provenance_postgres_source_unvalidated
```

`execution` is empty and every validation flag is false. The source guard and PostgreSQL harness are intentionally not run in this slice.

## Updated provenance matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Publish-operation after-save provenance owner | Source-ready | Runtime evidence pending |
| Exact body `updated_at` source revision | Harness-ready | PostgreSQL execution pending |
| Exact two locales retained in locale order | Harness-ready | PostgreSQL execution pending |
| Body/artifact/materialization identity binding | Harness-ready | PostgreSQL execution pending |
| `artifact_set_hash` mismatch rollback | Harness-ready | PostgreSQL execution pending |
| `sanitized_set_hash` mismatch rollback | Harness-ready | PostgreSQL execution pending |
| Aggregate mismatch adds no receipt/manifest/source | Harness-ready | PostgreSQL execution pending |
| Provenance survives referenced artifact row loss | Harness-ready | PostgreSQL execution pending |
| Provenance migration has no artifact FK | Guard-ready | Source guard execution pending |
| Legacy publish receipts receive no migration backfill | Guard-ready | Source guard / historical migration evidence pending |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Execute this provenance source guard and PostgreSQL harness and retain accepted exact-capture, rollback and artifact-loss evidence.
2. Execute the already source-ready SQLite/PostgreSQL immutable-artifact audit packets.
3. Execute the repair transport/request, PostgreSQL atomicity, negative SQLite and after-commit cache packets.
4. Retain successful bounded audit/repair transport evidence with current-tenant and Pages Manage fencing.
5. Execute the broader artifact/HTTP/browser and tenant Wave packets before FFA/FBA promotion.
6. Keep automatic audit-to-rebuild and rebuild-to-activation chaining absent until accepted execution evidence supports any policy change.

## Maintainer validation

Suggested commands, intentionally not run here:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance-postgres.mjs
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test publish_rebuild_provenance_postgres -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance.mjs
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo commands, formatting, PostgreSQL scenarios, GraphQL/HTTP requests, workflows and CI were intentionally not run.
