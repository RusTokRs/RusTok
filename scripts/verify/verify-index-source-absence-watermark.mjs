#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  contract: "crates/rustok-index/src/application/source_absence.rs",
  applicationMod: "crates/rustok-index/src/application/mod.rs",
  productProvider: "crates/rustok-distribution/src/product_index/absence.rs",
  productMod: "crates/rustok-distribution/src/product_index/mod.rs",
  reader: "crates/rustok-index/src/infrastructure/postgres/drift_snapshot_reader.rs",
  diagnosis: "apps/server/src/services/index_drift_diagnosis_operator.rs",
  doc: "crates/rustok-index/docs/m6-explicit-source-absence-watermark.md",
  readerDoc: "crates/rustok-index/docs/m6-postgres-drift-snapshot-reader.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-03.md",
};

const c = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([name, path]) => [name, await readFile(path, "utf8")]),
  ),
);

function requireMarkers(name, markers) {
  for (const marker of markers) {
    if (!c[name].includes(marker)) throw new Error(`${files[name]} missing ${marker}`);
  }
}

requireMarkers("contract", [
  "pub struct IndexSourceAbsenceWatermark",
  "pub trait IndexSourceAbsenceProvider",
  "pub struct IndexSourceAbsenceCatalog",
  "pub struct SharedIndexSourceAbsenceRegistry",
  "pub fn register_index_source_absence_provider",
  "pub fn materialize_index_source_absence_registry",
  "if source_version == 0",
  "watermark.key() != &expected",
  "source.owner_module() != descriptor.owner_module",
  "SchemaIdentityProviderConflict",
  "MissingSourceRegistry",
  "WatermarkScopeMismatch",
  "ProviderFailure",
]);

requireMarkers("applicationMod", [
  "mod source_absence;",
  "IndexSourceAbsenceProvider",
  "IndexSourceAbsenceWatermark",
  "SharedIndexSourceAbsenceRegistry",
  "materialize_index_source_absence_registry",
  "register_index_source_absence_provider",
]);

const contractProduction = c.contract.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "sea_orm",
  "DatabaseConnection",
  "SELECT ",
  "INSERT ",
  "UPDATE ",
  "DELETE FROM",
  "tokio::spawn",
  "spawn_blocking",
  ".scan(",
  "index_entities",
  "index_links",
  "repair_finding",
]) {
  if (contractProduction.includes(forbidden)) {
    throw new Error(`absence watermark contract contains forbidden marker: ${forbidden}`);
  }
}

requireMarkers("productProvider", [
  '"product-locale-absence-watermark"',
  '"product-locale-absence-postgres"',
  "impl IndexSourceAbsenceProvider for ProductLocaleAbsenceProvider",
  "register_index_source_absence_provider(",
  "products product",
  "CAST(projection.projection_epoch AS TEXT) AS source_version_text",
  "NOT EXISTS (",
  "FROM product_translations translation",
  "translation.locale = $3",
  "FROM product_index_tombstones tombstone",
  "tombstone.locale = $3",
  "IndexSourceAbsenceWatermark::new(key, source_version)",
  'retryable("product_index_absence_storage_unavailable")',
]);
requireMarkers("productMod", [
  "mod absence;",
  "absence::register(extensions)",
  "PRODUCT_ABSENCE_WATERMARK_FACTORY",
  "assert_eq!(factories.len(), 3)",
]);

const providerProduction = c.productProvider.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "INSERT ",
  "UPDATE ",
  "DELETE FROM",
  "tokio::spawn",
  "spawn_blocking",
  ".scan(",
  "index_entities",
  "index_links",
  "repair_finding",
]) {
  if (providerProduction.includes(forbidden)) {
    throw new Error(`Product absence provider contains forbidden marker: ${forbidden}`);
  }
}

requireMarkers("reader", [
  "SharedIndexSourceAbsenceRegistry",
  "with_absence_registry",
  "load_source_observation",
  "absence.provider_for_schema(&request.key().schema)",
  ".load(request.key().clone())",
  "IndexDriftSourceObservation::missing(",
  "let observed_again = match self.load_source_observation(request).await {",
  "source.absence_source_version.is_some()",
  "error.code() == SOURCE_WATERMARK_MISSING",
  "return Err(retryable_failure(SOURCE_CHANGED));",
  "if &observed_again != source",
  'b"explicit_source_absence_watermark_v1"',
  "source.absence_source_version",
  "source_version.to_be_bytes()",
  "index_drift_source_watermark_missing",
]);
requireMarkers("diagnosis", [
  "materialize_index_source_absence_registry(extensions)",
  "extensions.insert(absence);",
  "SharedIndexSourceAbsenceRegistry",
  "reader.with_absence_registry(absence)",
]);

requireMarkers("doc", [
  "source_complete_owner_execution_pending",
  "An empty targeted owner load is not proof that an entity is absent.",
  "`product-locale-absence-postgres`",
  "positive `products.index_revision`",
  "Product storage increments `index_revision` when translations are inserted, deleted, or reassigned.",
  "reloads the ordinary source and the absence watermark",
  "explicit_source_absence_watermark_v1",
  "index_drift_source_watermark_missing",
]);
requireMarkers("readerDoc", [
  "SharedIndexSourceAbsenceRegistry",
  "reload the exact absence watermark",
  "Product locale provider",
  "Existing\nUpsert/Delete boundary derivation is unchanged",
]);
requireMarkers("plan", [
  "M6 explicit source absence watermark registry and Product locale provider",
  "source_complete_owner_execution_pending",
  "Register Product locale absence",
]);

for (const claim of [
  "tests passed",
  "retained evidence admitted",
  "diagnosis transport is complete",
  "repair is complete",
]) {
  if (
    c.doc.toLowerCase().includes(claim.toLowerCase()) ||
    c.readerDoc.toLowerCase().includes(claim.toLowerCase())
  ) {
    throw new Error(`documentation makes forbidden completion claim: ${claim}`);
  }
}

console.log("Index explicit source absence watermark and Product wiring verified");
