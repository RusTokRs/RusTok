#!/usr/bin/env node
// Read-only supplementary source checks. No Rust, SQL, tests or migrations are executed.
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";
import { containsMarker, spaced } from "./lib/source-text.mjs";
import { rustFunctionBody } from "./verify-cart-priced-write-context.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
export const sourceFiles = {
  bootstrap: "crates/modules/rustok-inventory/src/services/bootstrap.rs",
  plan: "crates/modules/rustok-inventory/src/services/bootstrap/deletion.rs",
  pure: "crates/modules/rustok-inventory/src/services/bootstrap/deletion/tests.rs",
  inventoryRoot: "crates/modules/rustok-inventory/src/lib.rs",
  inventoryServices: "crates/modules/rustok-inventory/src/services/mod.rs",
  productError: "crates/modules/rustok-product/src/error.rs",
  productPublic: "crates/modules/rustok-product/src/public_error.rs",
  productCommandPort:
    "crates/modules/rustok-product/src/catalog_command_port.rs",
  commands: "crates/modules/rustok-product/src/services/catalog/commands.rs",
  http: "crates/modules/rustok-commerce/src/controllers/products.rs",
  storefront:
    "crates/modules/rustok-commerce/src/controllers/store/products.rs",
  graphql: "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs",
  migration:
    "crates/modules/rustok-inventory/src/migrations/m20261101_000026_index_catalog_deletion_history.rs",
  migrations: "crates/modules/rustok-inventory/src/migrations/mod.rs",
  fixtures: "crates/modules/rustok-commerce/tests/support/mod.rs",
  target: "crates/modules/rustok-commerce/tests/catalog_service_test.rs",
  persistence:
    "crates/modules/rustok-commerce/tests/inventory_catalog_deletion.rs",
  packet:
    "crates/modules/rustok-inventory/contracts/evidence/catalog-deletion-source.json",
  registry:
    "crates/modules/rustok-inventory/contracts/inventory-fba-registry.json",
  productRegistry:
    "crates/modules/rustok-product/contracts/product-fba-registry.json",
  companion: "scripts/verify/verify-inventory-catalog-deletion.test.mjs",
  package: "package.json",
};
export const adapterPaths = [
  "catalog_command_port.rs",
  "catalog_schema_read_port.rs",
  "catalog_schema_write_port.rs",
  "image_translation_progress_target.rs",
  "image_translation_target.rs",
  "media_reference.rs",
  "storefront_tag_read_port.rs",
  "translation_target.rs",
  "variant_translation_progress_target.rs",
  "variant_translation_target.rs",
  "ports/diagnostics.rs",
  "services/catalog_schema_service/attributes/translation.rs",
  "services/catalog_schema_service/categories/seo_translation.rs",
  "services/catalog_schema_service/categories/translation.rs",
  "services/catalog_schema_service/schemas/translation.rs",
  "services/catalog_schema_service/values/translation.rs",
  "services/catalog_schema_service/values/variant_translation.rs",
].map((file) => `crates/modules/rustok-product/src/${file}`);
export const historicalMigrations = [
  "m20250130_000016_create_commerce_inventory.rs",
  "m20260411_000001_add_stock_location_translations.rs",
  "m20260713_000017_enforce_reservation_identity.rs",
  "m20260713_000018_enforce_inventory_state_invariants.rs",
  "m20260713_000019_reserve_checkout_order_inventory.rs",
  "m20260713_000020_remove_duplicate_checkout_reservation.rs",
  "m20260713_000021_bound_reservation_metadata_and_inactive_locations.rs",
  "m20260713_000022_repair_oversized_reservation_metadata.rs",
  "m20260912_000023_add_stock_location_translation_change_journal.rs",
  "m20261021_000024_create_inventory_return_restocks.rs",
  "m20261101_000025_create_shipment_stock_proofs.rs",
].map((file) => `crates/modules/rustok-inventory/src/migrations/${file}`);
export const sourcePaths = [
  ...new Set([
    ...Object.values(sourceFiles),
    ...adapterPaths,
    ...historicalMigrations,
  ]),
];

export function verifyInventoryCatalogDeletion({ root = repository } = {}) {
  const failures = [];
  const read = (file) => {
    try {
      return fs.readFileSync(path.join(root, file), "utf8");
    } catch {
      failures.push(`missing source: ${file}`);
      return "";
    }
  };
  const sources = Object.fromEntries(
    Object.entries(sourceFiles).map(([key, file]) => [key, read(file)]),
  );
  const runtime = (source) => source.split(/#\[cfg\(test\)\]/)[0];
  const plan = runtime(sources.plan);
  const require = (source, marker, label) => {
    if (!containsMarker(source, marker))
      failures.push(`${label}: missing ${marker}`);
  };
  const forbid = (source, marker, label) => {
    if (containsMarker(source, marker))
      failures.push(`${label}: forbidden ${marker}`);
  };
  const body = (source, name) => {
    const found = rustFunctionBody(source, name);
    if (!found) failures.push(`missing function ${name}`);
    return found ?? "";
  };
  const before = (source, first, second, label) => {
    const text = spaced(source);
    const locate = (marker) =>
      marker instanceof RegExp
        ? text.search(marker)
        : text.indexOf(spaced(marker));
    const a = locate(first),
      b = locate(second);
    if (a < 0 || b < 0 || a >= b) failures.push(`${label}: invalid ordering`);
  };

  require(sources.bootstrap, /pub async fn delete_records_for_variants_in_tx\s*\(\s*txn:\s*&sea_orm::DatabaseTransaction,\s*tenant_id:\s*Uuid,/, "concrete transaction/current tenant");
  require(body(
    sources.bootstrap,
    "delete_records_for_variants_in_tx",
  ), "deletion::delete_records(txn, tenant_id, actor_id, variant_ids).await", "single owner implementation");
  for (const source of [runtime(sources.bootstrap), plan]) {
    for (const marker of [
      "reservation_item::Entity::delete",
      "ledger::Entity::delete",
      "proof::Entity::delete",
      "saturating_",
      "DomainEvent::InventoryUpdated",
    ]) {
      // Other bootstrap reads retain existing saturating arithmetic; it is not deletion proof.
      if (marker === "saturating_" && source !== plan) continue;
      forbid(
        source,
        marker,
        "no destructive history/fabricated quantity fallback",
      );
    }
  }
  require(plan, "pub const MAX_CATALOG_DELETE_VARIANTS: usize = 256;", "SKU batch bound");
  require(plan, "const MAX_DELETE_LEVELS: usize = 4096;", "complete level bound");
  const entry = body(plan, "delete_records");
  before(
    entry,
    "validate_request(tenant_id, variant_ids)?",
    "if variant_ids.is_empty()",
    "tenant/identity validation before noop",
  );
  before(
    entry,
    "DeletionPlan::load",
    "plan.apply(txn, tenant_id, actor_id)",
    "whole plan before effects",
  );
  const request = body(plan, "validate_request");
  for (const marker of [
    "tenant_id.is_nil()",
    "variant_ids.iter().any(Uuid::is_nil)",
    "variant_ids.len() > MAX_CATALOG_DELETE_VARIANTS",
    "ids.len() != variant_ids.len()",
    "BTreeSet<_>",
  ])
    require(request, marker, "request proof");
  require(body(
    plan,
    "require_current_snapshot",
  ), 'isolation != "read committed"', "PostgreSQL current snapshot");
  require(body(
    plan,
    "current_read",
  ), "query.lock_exclusive()", "shared current item/history/level locks");
  const load = body(plan, "load");
  for (const marker of [
    "product_variant::Column::TenantId.eq(tenant_id)",
    "variants.as_slice() != variant_ids",
    "items.len() != variant_ids.len()",
    "item_ids.len() != items.len()",
    "item_ids.iter().any(Uuid::is_nil)",
    "inventory_item::Column::VariantId",
    "has_history(txn, variant_ids, &item_ids).await?",
    "discovered.len() > MAX_DELETE_LEVELS",
    "levels.len() > MAX_DELETE_LEVELS",
    "map(Level::binding)",
    "validate_levels(&levels, &item_ids, &location_ids)?",
  ])
    require(load, marker, "complete owner plan");
  before(
    load,
    "product_variant::Entity::find()",
    "inventory_item::Entity::find()",
    "tenant proof before Inventory",
  );
  before(
    load,
    "inventory_item::Column::VariantId",
    "has_history(",
    "shared item lock before history",
  );
  before(
    load,
    "stock_location::Entity::find()",
    /current_read\(\s*txn,\s*level_query\(&item_ids\)\s*\)/,
    "original location locks before current levels",
  );
  forbid(
    load,
    "stock_location::Column::DeletedAt",
    "empty archived locations are not reactivated/reselected",
  );
  const history = body(plan, "has_history");
  for (const marker of [
    "reservation_item::Column::InventoryItemId",
    "ledger::Column::InventoryItemId",
    "proof::Column::VariantId",
    "proof::Column::InventoryItemId",
    "current_read(",
    "select_only()",
    "limit(1)",
    "into_tuple::<Uuid>()",
  ])
    require(history, marker, "all retained history references");
  for (const marker of [
    "Column::DeletedAt",
    "Column::Quantity",
    "Column::ShippedQuantity",
    "Column::RestoredQuantity",
    "Column::Metadata",
    "Column::TenantId.eq(",
  ])
    forbid(
      history,
      marker,
      "history state/contradictory labels cannot authorize deletion",
    );
  require(body(
    plan,
    "is_empty",
  ), "self.stocked_quantity == 0 && self.reserved_quantity == 0 && self.incoming_quantity == 0", "individual zero counters");
  require(body(
    plan,
    "level_query",
  ), "limit((MAX_DELETE_LEVELS + 1) as u64)", "overflow sentinel not truncation");
  const apply = body(plan, "apply");
  for (const marker of [
    "inventory_level::Column::Id.eq(level.id)",
    "inventory_level::Column::InventoryItemId.eq(level.inventory_item_id)",
    "inventory_level::Column::LocationId.eq(level.location_id)",
    "inventory_level::Column::StockedQuantity.eq(0)",
    "inventory_level::Column::ReservedQuantity.eq(0)",
    "inventory_level::Column::IncomingQuantity.eq(0)",
    "inventory_item::Column::Id.eq(item_id)",
    "inventory_item::Column::VariantId.eq(variant_id)",
  ])
    require(apply, marker, "exact conditional effects");
  if ((spaced(apply).match(/deleted\.rows_affected != 1/g) ?? []).length !== 2)
    failures.push("both level and item effects require exact-one admission");
  for (const marker of [
    "commit(",
    "unwrap_or",
    "let _ =",
    ".ok()",
    "repair",
    "backfill",
  ])
    forbid(apply, marker, "caller-owned rollback/no swallowing");

  const removeProduct = body(sources.commands, "delete_product");
  const removeVariant = body(sources.commands, "delete_variant");
  for (const source of [removeProduct, removeVariant]) {
    require(source, /BootstrapService::delete_records_for_variants_in_tx\(\s*&txn,\s*tenant_id,\s*actor_id,/, "real Product caller tenant");
    before(
      source,
      "BootstrapService::delete_records_for_variants_in_tx",
      "PricingBootstrapService::delete_prices_for_variants_in_tx",
      "Inventory refusal before peer cleanup",
    );
    require(source, "CatalogDeletionBlocker::ConcurrentChange", "exact final catalog effects");
  }
  for (const marker of [
    "limit((rustok_inventory::MAX_CATALOG_DELETE_VARIANTS + 1) as u64)",
    "variants.len() > rustok_inventory::MAX_CATALOG_DELETE_VARIANTS",
    "variants.iter().any(|(_, owner)| *owner != tenant_id)",
    "if variants.is_empty()",
    "deleted_variants.rows_affected != variant_ids.len() as u64",
    "deleted_product.rows_affected != 1",
  ])
    require(removeProduct, marker, "whole Product target proof");
  require(removeVariant, "deleted_variant.rows_affected != 1", "standalone SKU exact effect");
  require(removeVariant, "CommerceError::CannotDeleteOnlyVariant", "last-SKU invariant retained");

  for (const file of adapterPaths) {
    const source = runtime(read(file));
    require(source, "CommerceError::InventoryDeletionBlocked(blocker)", `${file} exhaustive typed refusal`);
    require(source, "crate::error::inventory_deletion_port_error(&blocker)", `${file} canonical policy`);
  }
  require(sources.storefront, "ProductError::InventoryDeletionBlocked(_)", "unexpected read failure is closed");
  require(sources.productPublic, "crate::error::inventory_deletion_public_policy(blocker)", "native public error policy");
  const reverse = body(
    runtime(sources.productError),
    "inventory_deletion_blocker",
  );
  require(reverse, "error.kind == kind && error.code == code", "closed code and kind match");
  forbid(reverse, "error.message", "no provider-text classification");
  require(body(
    sources.productCommandPort,
    "product_catalog_command_error_is_terminal",
  ), "crate::error::inventory_deletion_blocker(error).is_some()", "existing finalized-receipt classification");
  require(sources.http, "rustok_product::error::inventory_deletion_public_failure", "HTTP safe copy");
  require(sources.http, "response.with_retryable(false)", "native refusal HTTP metadata");
  require(sources.graphql, "rustok_product::error::inventory_deletion_public_failure(&error)", "GraphQL safe copy");
  require(sources.graphql, "product_command_error_outcome(&error)", "presentation is not outcome authority");
  forbid(
    body(sources.graphql, "product_command_port_error"),
    "internal_code = %error.code",
    "bounded provider diagnostics",
  );

  for (const name of ["m20261101_000026_index_catalog_deletion_history"]) {
    require(sources.migrations, `mod ${name};`, "real migration registration");
    require(sources.migrations, `Box::new(${name}::Migration)`, "live migrator");
    require(sources.fixtures, `"${name}"`, "real migration fixture");
  }
  for (const marker of [
    "idx_inventory_catalog_delete_returns",
    "idx_inventory_catalog_delete_shipment_variant",
    "idx_inventory_catalog_delete_shipment_item",
  ])
    require(sources.migration, marker, "live history reference index");
  for (const marker of [
    "drop_table",
    "alter_table",
    "delete_many",
    "UPDATE ",
    "DELETE FROM",
    "return_item_id",
  ])
    forbid(sources.migration, marker, "additive indexes only");
  require(sources.target, '#[path = "inventory_catalog_deletion.rs"]', "registered persistence regressions");
  require(sources.target, "mod inventory_catalog_deletion;", "compiled test target inclusion");

  let rustDefinitions = 0;
  try {
    const packet = JSON.parse(sources.packet);
    for (const field of [
      "compiled",
      "sql_executed",
      "migrations_executed",
      "runtime_certified",
      "generic_index_complete",
      "step8_complete",
      "tests_executed",
    ])
      if (packet[field] !== false)
        failures.push(`${field}: source packet cannot promote evidence`);
    if (
      packet.schema_version !== 1 ||
      packet.module !== "inventory" ||
      packet.packet !== "catalog_deletion_source" ||
      packet.status !== "source_implementation_uncompiled_unrun"
    )
      failures.push("native source-only packet identity/status drift");
    if (
      packet.source_regressions.executed !== false ||
      packet.source_regressions.compiled !== false
    )
      failures.push("regression compilation/execution must remain unclaimed");
    if (
      packet.source_regressions.rust_definitions !== 31 ||
      packet.source_regressions.node_definitions !== 10
    )
      failures.push(
        "declared regression definition counts must match source scope",
      );
    for (const group of packet.source_regressions.rust) {
      const source = read(group.path);
      for (const name of group.definitions) {
        require(source, new RegExp(
          `#\\[(?:tokio::)?test\\]\\s*(?:async\\s+)?fn\\s+${name}\\s*\\(`,
        ), `regression definition ${name}`);
        rustDefinitions += 1;
      }
    }
    if (rustDefinitions !== 31)
      failures.push(
        `expected 31 new Rust definitions, found ${rustDefinitions}`,
      );
    if ((sources.companion.match(/^test\s*\(/gm) ?? []).length !== 10)
      failures.push("expected 10 unrun companion definitions");
    for (const file of historicalMigrations) {
      const current = crypto
        .createHash("sha256")
        .update(read(file))
        .digest("hex");
      if (packet.historical_inventory_migrations[file] !== current)
        failures.push(`historical migration changed: ${file}`);
    }
    for (const source of [sources.registry, sources.productRegistry])
      require(source, sourceFiles.packet, "owner source evidence link");
  } catch (error) {
    failures.push(`invalid source-only packet: ${error.message}`);
  }
  require(sources.package, '"verify:inventory:catalog-deletion"', "read-only guard alias");
  return {
    status: failures.length ? "failed" : "passed_source_only",
    failures,
    rust_definitions: rustDefinitions,
    node_definitions: 10,
    tests_executed: false,
    compiled: false,
    sql_executed: false,
    generic_index_complete: false,
    step8_complete: false,
  };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const result = verifyInventoryCatalogDeletion();
  if (result.failures.length) {
    result.failures.forEach((failure) => console.error(`✖ ${failure}`));
    process.exitCode = 1;
  } else
    console.log(
      "✔ Inventory catalog deletion source contract; 31 Rust/10 Node definitions unrun; compiler, SQL, migration, concurrency and generic Index remain pending",
    );
}
