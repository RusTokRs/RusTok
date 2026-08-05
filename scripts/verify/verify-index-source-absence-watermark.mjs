#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  contract: "crates/rustok-index/src/application/source_absence.rs",
  applicationMod: "crates/rustok-index/src/application/mod.rs",
  reader: "crates/rustok-index/src/infrastructure/postgres/drift_snapshot_reader.rs",
  doc: "crates/rustok-index/docs/m6-explicit-source-absence-watermark.md",
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

const production = c.contract.split("\n#[cfg(test)]")[0];
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
  if (production.includes(forbidden)) {
    throw new Error(`absence watermark contract contains forbidden marker: ${forbidden}`);
  }
}

requireMarkers("reader", [
  "index_drift_source_watermark_missing",
  "if mutations.is_empty()",
]);
if (c.reader.includes("SharedIndexSourceAbsenceRegistry")) {
  throw new Error("snapshot reader wiring is intentionally pending in this contract-only slice");
}

requireMarkers("doc", [
  "source_complete_owner_registration_and_reader_wiring_pending",
  "An empty targeted owner load is not proof that an entity is absent.",
  "one positive `source_version`",
  "same\nowner as the canonical replay source",
  "`None` remains non-authoritative",
  "wire the frozen absence registry into\n`PostgresIndexDriftSnapshotReader`",
]);

requireMarkers("plan", [
  "M6 explicit source absence watermark registry",
  "source_complete_owner_registration_and_reader_wiring_pending",
  "wire the frozen absence registry into the PostgreSQL drift snapshot reader",
]);

for (const claim of [
  "tests passed",
  "production owner provider is complete",
  "snapshot reader absence wiring is complete",
  "retained evidence admitted",
  "repair is complete",
]) {
  if (c.doc.toLowerCase().includes(claim.toLowerCase())) {
    throw new Error(`documentation makes forbidden completion claim: ${claim}`);
  }
}

console.log("Index explicit source absence watermark contract verified");
