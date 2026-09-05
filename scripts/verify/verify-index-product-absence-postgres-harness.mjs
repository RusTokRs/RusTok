#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  test: "crates/rustok-distribution/tests/product_locale_absence_postgres.rs",
  provider: "crates/rustok-distribution/src/product_index/absence.rs",
  reader: "crates/rustok-index/src/infrastructure/postgres/drift_snapshot_reader.rs",
  productMigration:
    "crates/rustok-product/src/migrations/m20260731_000004_add_product_index_tombstones.rs",
  canonicalProjectionMigration:
    "crates/rustok-product/src/migrations/m20260807_000010_canonicalize_product_index_graph_projection.rs",
  freshnessMigration:
    "crates/rustok-product/src/migrations/m20260807_000011_add_product_sales_channel_relation_freshness.rs",
  doc: "crates/rustok-index/docs/m6-product-locale-absence-postgres-harness.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-03.md",
  aggregate: "scripts/verify/verify-index-query-contract.mjs",
};

const content = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([name, path]) => [name, await readFile(path, "utf8")]),
  ),
);

function requireMarkers(name, markers) {
  for (const marker of markers) {
    if (!content[name].includes(marker)) {
      throw new Error(`${files[name]} missing ${marker}`);
    }
  }
}

requireMarkers("test", [
  '#![cfg(feature = "mod-product")]',
  "struct ProductMigrator;",
  "rustok_product::migrations::migrations()",
  "for migration in IndexModule.migrations()",
  "create_product_migration_prerequisites",
  "flex::cache_generation::create_field_definition_cache_generation_table",
  "CREATE TABLE channel_index_identity_generations",
  "rustok_distribution::build_runtime_extensions(&registry)",
  "materialize_postgres_index_sources",
  "materialize_index_source_registry",
  "materialize_index_source_absence_registry",
  "materialize_postgres_index_drift_snapshot_reader",
  "assert_missing(stable.source(), &stable_key)",
  "assert_missing(stable.materialized(), &stable_key)",
  "LOCK TABLE index_entities IN ACCESS EXCLUSIVE MODE",
  "FROM pg_stat_activity",
  "application_name = $1",
  "query LIKE '%FROM index_entities WHERE tenant_id%'",
  "insert_de_translation",
  "INSERT INTO product_translations",
  "INSERT INTO product_sales_channel_index_relation_snapshots",
  "INSERT INTO product_sales_channel_index_relation_freshness_snapshots",
  "product.index_revision",
  "'all'",
  "'[]'::jsonb",
  "SchemaVersion::new(4)",
  "translation insertion between observations must reject the snapshot pair",
  "IndexDriftDependencyFailureKind::Retryable",
  '"index_drift_source_changed_during_capture"',
  "DROP SCHEMA IF EXISTS",
]);

for (const forbidden of [
  "SchemaVersion::INITIAL",
  "SchemaVersion::new(3)",
  "SequencedSource",
  "FixedProvider",
  "register_index_source_absence_provider",
  "IndexSourceAbsenceWatermark::new",
  "tokio::time::timeout(Duration::from_secs(0)",
]) {
  if (content.test.includes(forbidden)) {
    throw new Error(`Product absence PostgreSQL harness contains forbidden shortcut or historical key: ${forbidden}`);
  }
}

requireMarkers("provider", [
  "impl IndexSourceAbsenceProvider for ProductLocaleAbsenceProvider",
  "CAST(projection.projection_epoch AS TEXT) AS source_version_text",
  "FROM product_index_graph_projection_snapshots projection",
  "projection.product_source_version = product.index_revision",
  "JOIN product_sales_channel_index_relation_snapshots relation",
  "product_sales_channel_index_relation_freshness_snapshots",
  "channel_index_identity_generations",
  "decode_product_visibility(&metadata)",
  "freshness_visibility_key != current_visibility_key",
  "freshness_channel_identity_generation != current_channel_identity_generation",
  "FROM product_translations translation",
  "FROM product_index_tombstones tombstone",
]);
requireMarkers("reader", [
  "load_source_observation",
  "let observed_again = match self.load_source_observation(request).await",
  "index_drift_source_changed_during_capture",
  "explicit_source_absence_watermark_v1",
]);
requireMarkers("productMigration", [
  "rustok_product_translation_bump_index_revision",
  "rustok_product_store_index_tombstone",
  "rustok_product_clear_superseded_index_tombstone",
]);
requireMarkers("canonicalProjectionMigration", [
  "product_index_graph_projection_snapshots",
  "rustok_product_reconcile_index_graph_projection",
  "trg_product_channel_relation_index_graph_projection_insert",
]);
requireMarkers("freshnessMigration", [
  "product_sales_channel_index_relation_freshness_snapshots",
  "visibility_key TEXT NOT NULL",
  "channel_identity_generation BIGINT NOT NULL",
]);
requireMarkers("doc", [
  "Status: `source_ready_owner_execution_pending`.",
  "real selected Product adapters and real owner migrations",
  "does not copy the Product provider query",
  "Stable absence scenario",
  "Deterministic translation race",
  "ACCESS EXCLUSIVE",
  "pg_stat_activity",
  "index_drift_source_changed_during_capture",
  "No retained execution evidence is admitted",
]);
requireMarkers("plan", [
  "Product locale absence PostgreSQL harness",
  "source_ready_owner_execution_pending",
  "product_locale_absence_postgres",
]);
requireMarkers("aggregate", [
  "'verify-index-product-absence-postgres-harness.mjs'",
]);

for (const claim of [
  "tests passed",
  "PostgreSQL execution passed",
  "retained evidence admitted",
  "production evidence complete",
]) {
  if (content.doc.toLowerCase().includes(claim.toLowerCase())) {
    throw new Error(`Product absence harness document makes forbidden claim: ${claim}`);
  }
}

console.log("Index Product locale absence PostgreSQL harness source verified");
