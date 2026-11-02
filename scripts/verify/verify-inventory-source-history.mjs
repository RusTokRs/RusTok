#!/usr/bin/env node
// Read-only lexical guard. Does not execute Rust, SQL, migrations or companion callbacks.
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";
import { containsMarker } from "./lib/source-text.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const owner = "crates/modules/rustok-inventory";
export const files = {
  api: `${owner}/src/source_history/mod.rs`,
  baseline: `${owner}/src/source_history/baseline.rs`,
  reader: `${owner}/src/source_history/reader.rs`,
  writer: `${owner}/src/source_history/writer.rs`,
  snapshot: `${owner}/src/source_history/snapshot.rs`,
  model: `${owner}/src/source_history/model.rs`,
  pure: `${owner}/src/source_history/tests.rs`,
  physicalRegression: `${owner}/src/source_history/physical_tests.rs`,
  migration: `${owner}/src/migrations/m20261101_000027_create_inventory_source_history.rs`,
  registry: `${owner}/src/migrations/mod.rs`,
  root: `${owner}/src/lib.rs`,
  quantity: `${owner}/src/services/quantity.rs`,
  reservation: `${owner}/src/reservation_events.rs`,
  physical: `${owner}/src/stock_events.rs`,
  bootstrap: `${owner}/src/services/bootstrap.rs`,
  deletion: `${owner}/src/services/bootstrap/deletion.rs`,
  adoption: `${owner}/src/reservation_adoption.rs`,
  persistence:
    "crates/modules/rustok-commerce/tests/inventory_source_history.rs",
  target: "crates/modules/rustok-commerce/tests/catalog_service_test.rs",
  fixture: "crates/modules/rustok-commerce/tests/support/mod.rs",
  packet: `${owner}/contracts/evidence/source-history-source.json`,
  companion: "scripts/verify/verify-inventory-source-history.test.mjs",
  package: "package.json",
};

export function verifyInventorySourceHistory({ root = repository } = {}) {
  const failures = [];
  const read = (name) => {
    try {
      return fs.readFileSync(path.join(root, name), "utf8");
    } catch {
      failures.push(`missing ${name}`);
      return "";
    }
  };
  const s = Object.fromEntries(
    Object.entries(files).map(([name, file]) => [name, read(file)]),
  );
  const require = (text, marker, label) => {
    if (!containsMarker(text, marker))
      failures.push(`${label}: missing ${marker}`);
  };
  for (const marker of [
    "MAX_SOURCE_PAGE: usize = 128",
    "MAX_SOURCE_LEVELS: usize = 4096",
    "Permission::INVENTORY_MANAGE",
    "tenant.is_nil()",
    "actor.is_nil()",
    "pg_try_advisory_xact_lock(",
    "pg_try_advisory_xact_lock_shared(",
    '"SHOW transaction_isolation"',
    "if !saved.active",
    "if stranded",
  ])
    require(s.api, marker, "positive scope, explicit baseline and nonblocking lock-order gate");
  for (const marker of [
    "request.baseline_id.is_nil()",
    "request.page_id.is_nil()",
    "page::Entity::find_by_id((tenant, request.page_id))",
    "receipt.after_variant_id != request.after_variant_id",
    "receipt.page_limit != request.limit as i32",
    "saved.last_variant_id != request.after_variant_id",
    "request.limit + 1",
    "level_count > MAX_SOURCE_LEVELS",
    "super::writer::insert",
    "baseline::Column::RecordCount.eq(saved.record_count)",
    "update.exec(txn).await?.rows_affected != 1",
    "page::Entity::insert",
    "if affected != 1",
    "txn.rollback().await?",
    "with_timeout",
    "load_with_budget",
  ])
    require(s.baseline, marker, "complete paged baseline, immutable receipt and atomic checkpoint");
  for (const marker of [
    "product_variant::Column::TenantId.eq(tenant)",
    "product::Column::TenantId.eq(tenant)",
    "items.len() != 1",
    "allowance + 1",
    "levels.len() > allowance",
    "MAX_SOURCE_LEVELS + 1",
    "stock_location::Column::TenantId.eq(tenant)",
    "locations.insert(level.location_id)",
    "level.incoming_quantity < 0",
    "checked_add(i64::from",
    "sha256_digest",
  ])
    require(s.snapshot, marker, "bounded positive original snapshot");
  for (const marker of [
    "super::tracking_baseline",
    "source_version: Set(1)",
    "next_version(saved.source_version)",
    "saved.binding_digest != snapshot.binding_digest",
    "change::Entity::find_by_id(saved.last_change_id)",
    "record::Column::SourceVersion.eq(saved.source_version)",
    "record::Column::LastChangeId.eq(saved.last_change_id)",
    "result.rows_affected != 1",
    "if affected != 1",
    '"deleted"',
    "PreparedSourceDeletion<'a>",
    "conn: &'a DatabaseTransaction",
    "update(self.conn, saved, 0, 0, 0, true, self.actor)",
    "map_err(E::into_db_error)",
  ])
    require(s.writer, marker, "positive exact versions, provenance and retained deletion");
  for (const marker of [
    "if rows.len() != unique.len()",
    "Err(E::StateUnverified)",
    "validate_change",
    "head.source_version != row.source_version",
    'row.deleted != (head.kind == "deleted")',
    "row.stocked.checked_sub(row.reserved)",
  ])
    require(s.reader, marker, "targeted missing is unverified and head is exact");
  for (const [name, marker] of [
    [
      "quantity",
      /record_changed\(\s*txn,\s*self\.tenant_id,\s*self\.variant_id,\s*Some\(self\.actor_id\),?\s*\)/,
    ],
    [
      "reservation",
      "record_changed(txn, self.tenant_id, *variant_id, self.actor_id)",
    ],
    ["physical", "self.bindings.values()"],
    [
      "physical",
      "record_changed(txn, self.tenant_id, variant_id, self.actor_id)",
    ],
    [
      "bootstrap",
      /record_created\(\s*conn,\s*location\.tenant_id,\s*input\.variant_id,\s*Some\(actor_id\),?\s*\)/,
    ],
    ["bootstrap", "conn: &sea_orm::DatabaseTransaction"],
    [
      "deletion",
      /prepare_deletion\(\s*txn,\s*tenant_id,\s*variant_id,\s*item_id,\s*Some\(actor_id\),?\s*\)/,
    ],
    ["deletion", "deletion.record_deleted()"],
  ])
    require(s[name], marker, `actual ${name} owner writer`);
  if (containsMarker(s.adoption, "source_history::record_"))
    failures.push("adoption-only metadata must not advance stock source");
  for (const text of [s.writer, s.snapshot])
    if (containsMarker(text, "saturating_"))
      failures.push("source counters must not saturate");
  const production = [
    s.api,
    s.baseline,
    s.reader,
    s.writer,
    s.snapshot,
    s.model,
  ].join("\n");
  for (const forbidden of [
    "product_variant::Column::IndexRevision",
    "product_index_variant_refresh",
    "INSERT INTO index_",
    "UPDATE index_",
    "DELETE FROM index_",
    "unwrap()",
    "expect(",
  ])
    if (containsMarker(production, forbidden))
      failures.push(`owner boundary: forbidden ${forbidden}`);
  for (const text of [s.registry, s.fixture])
    require(text, "m20261101_000027_create_inventory_source_history", "real additive migration and fixture registration");
  require(s.migration, "retained baselines, history and tombstones survive", "nonempty downgrade refuses");
  require(s.migration, "inventory_source_baseline_pages", "durable original page outcomes");
  require(s.root, "pub mod source_history", "owner API export");
  require(s.target, "mod inventory_source_history", "real regression target");
  require(s.package, '"verify:inventory:source-history"', "read-only command discovery");
  let packet = {};
  try {
    packet = JSON.parse(s.packet);
  } catch {
    failures.push("invalid source packet");
  }
  if (
    packet.packet !== "inventory-source-history-source" ||
    packet.status !== "source_only_uncompiled_unrun"
  )
    failures.push("packet identity/status drift");
  for (const key of [
    "compiled",
    "tests_executed",
    "sql_executed",
    "migrations_executed",
    "runtime_verified",
    "installation_baseline_initialized",
    "canonical_versioned_publication_wired",
    "distribution_selected",
    "index_ready",
    "step8_complete",
  ])
    if (packet[key] !== false) failures.push(`must not promote ${key}`);
  const bounds = packet.bounds ?? {};
  if (
    bounds.variants_per_page !== 128 ||
    bounds.complete_levels_per_page !== 4096 ||
    bounds.level_rows_per_rejected_page_max !== 4097 ||
    bounds.source_version_max !== "9223372036854775807" ||
    JSON.stringify(bounds.context_budget_ms) !== "[1,30000]" ||
    packet.regressions?.executed !== false
  )
    failures.push("bounded source and unrun regression evidence drift");
  let definitions = 0;
  for (const [file, names] of Object.entries(packet.regressions?.rust ?? {}))
    for (const name of names) {
      require(read(file), `fn ${name}(`, "focused definition");
      definitions++;
    }
  if (
    definitions !== 18 ||
    packet.regressions?.rust_definitions !== 18 ||
    packet.regressions?.node_definitions !== 8 ||
    (s.companion.match(/^test\(/gm) ?? []).length !== 8
  )
    failures.push("regression inventory drift");
  for (const [file, expected] of Object.entries(packet.preserved_hashes ?? {}))
    if (
      crypto.createHash("sha256").update(read(file)).digest("hex") !== expected
    )
      failures.push(`immutable historical payload changed: ${file}`);
  if (Object.keys(packet.preserved_hashes ?? {}).length !== 14)
    failures.push("missing immutable historical hashes");
  return {
    failures,
    rust_definitions: definitions,
    node_definitions: 8,
    testsExecuted: false,
    runtimeVerified: false,
  };
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const result = verifyInventorySourceHistory();
  if (result.failures.length) {
    result.failures.forEach((message) => console.error(message));
    process.exitCode = 1;
  } else
    console.log(
      "Inventory source-history source guard passed; 18 Rust/8 Node definitions unrun; selected Distribution/Index consumer delivery and runtime remain pending; canonical producer continuation is separately tracked.",
    );
}
