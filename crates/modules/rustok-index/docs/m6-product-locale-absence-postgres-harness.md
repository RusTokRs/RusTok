# M6 Product locale absence PostgreSQL harness

Status: `source_ready_owner_execution_pending`.

## Purpose

`product_locale_absence_postgres` is an environment-gated integration harness for the production
Product locale absence provider and the PostgreSQL drift snapshot fence. It lives in
`rustok-distribution`, where the selected Product bridge, Product migrations, and generic Index
runtime composition are all available without exposing Product-specific types from Index core or
the executable server.

The harness does not replace the existing database-neutral registry tests or the generic
`drift_snapshot_reader_postgres_test`. It closes the source-evidence gap between those contracts by
using the real selected Product adapters and real owner migrations.

## Real migration boundary

The harness creates one isolated PostgreSQL schema and then:

1. creates only the documented external prerequisites used by the existing Product migration
   lifecycle harness: `tenants`, `taxonomy_terms`, and the Flex field-definition cache-generation
   table;
2. applies every migration returned by `rustok_product::migrations::migrations()` through a real
   `MigratorTrait` implementation;
3. applies every migration returned by `IndexModule::migrations()`;
4. inserts one real Product and one English Product translation;
5. builds distribution runtime extensions from `IndexModule` plus `ProductModule`;
6. materializes the production PostgreSQL Product, ProductVariant, and Product-locale absence
   factories;
7. freezes the canonical source registry and owner-bound absence registry;
8. constructs `PostgresIndexDriftSnapshotReader` through the public production materializer.

The test does not copy the Product provider query, construct a fake Product source, or inject a
test-only callback into production code.

## Stable absence scenario

The Product exists and has an English translation, while the requested French locale has neither a
live translation nor a retained locale tombstone. The ordinary targeted Product load is empty, and
`product-locale-absence-postgres` returns the positive `products.index_revision` watermark.

The reader must return one `pg:` snapshot pair whose source and materialized states are both the
exact requested `Missing` key. This proves only source readiness until the owner executes and
admits the harness output.

## Deterministic translation race

The race scenario uses a second absent locale and no production hook:

1. a separate PostgreSQL transaction takes `ACCESS EXCLUSIVE` on `index_entities`;
2. snapshot capture starts normally, completing the first ordinary Product load and first
   production absence-watermark read;
3. the harness waits through `pg_stat_activity` until the real snapshot reader is blocked on its
   exact `index_entities` materialized SELECT;
4. another connection inserts the requested Product translation;
5. the real Product translation trigger advances `products.index_revision`;
6. the materialized-table lock is released;
7. the reader performs its second ordinary Product load and observes the new live translation.

The pair must be rejected as retryable
`index_drift_source_changed_during_capture`. The lock is on Index materialized storage only, so it
does not prevent the owner translation write or falsify the owner revision transition.

## Isolation and cleanup

Every PostgreSQL role in the scenario uses a dedicated one-connection pool and an exact
`search_path` for the generated schema. The snapshot connection also receives a unique bounded
`application_name`, allowing the observer to identify only the intended blocked read. The schema is
dropped with `CASCADE` after the scenario, including when an assertion returns an error after setup.

The harness performs no production scheduling, transport publication, finding resolution, or
repair.

## Owner execution

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-distribution \
  --features mod-product \
  --test product_locale_absence_postgres \
  -- --nocapture --test-threads=1
```

Static source verification:

```bash
node scripts/verify/verify-index-product-absence-postgres-harness.mjs
```

The implementation agent did not run formatting, Cargo checks/tests, JavaScript verifiers,
PostgreSQL scenarios, workflows, or CI. No retained execution evidence is admitted by this source
slice.
