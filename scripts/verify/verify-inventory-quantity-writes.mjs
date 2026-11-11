#!/usr/bin/env node
// Supplementary read-only source checks, never Rust/SQL/test/runtime evidence.
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
export const files = {
  plan: "crates/modules/rustok-inventory/src/services/quantity.rs",
  pure: "crates/modules/rustok-inventory/src/services/quantity/tests.rs",
  native: "crates/modules/rustok-inventory/src/services/inventory.rs",
  bootstrap: "crates/modules/rustok-inventory/src/services/bootstrap.rs",
  visible: "crates/modules/rustok-inventory/src/services/public_channel.rs",
  root: "crates/modules/rustok-inventory/src/lib.rs",
  services: "crates/modules/rustok-inventory/src/services/mod.rs",
  productError: "crates/modules/rustok-product/src/error.rs",
  public: "crates/modules/rustok-product/src/public_error.rs",
  commandPort: "crates/modules/rustok-product/src/catalog_command_port.rs",
  transaction:
    "crates/modules/rustok-product/src/services/write_transaction.rs",
  commands: "crates/modules/rustok-product/src/services/catalog/commands.rs",
  projection:
    "crates/modules/rustok-product/src/services/catalog/projection.rs",
  http: "crates/modules/rustok-commerce/src/controllers/products.rs",
  graphql: "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs",
  persistence:
    "crates/modules/rustok-commerce/tests/inventory_quantity_writes.rs",
  target: "crates/modules/rustok-commerce/tests/catalog_service_test.rs",
  packet:
    "crates/modules/rustok-inventory/contracts/evidence/quantity-writes-source.json",
  registry:
    "crates/modules/rustok-inventory/contracts/inventory-fba-registry.json",
  productRegistry:
    "crates/modules/rustok-product/contracts/product-fba-registry.json",
  package: "package.json",
  companion: "scripts/verify/verify-inventory-quantity-writes.test.mjs",
};
export const adapters = [
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
].map((name) => `crates/modules/rustok-product/src/${name}`);

export function verifyInventoryQuantityWrites({ root = repository } = {}) {
  const failures = [];
  const read = (name) => {
    try {
      return fs.readFileSync(path.join(root, name), "utf8");
    } catch {
      failures.push(`missing source ${name}`);
      return "";
    }
  };
  const source = Object.fromEntries(
    Object.entries(files).map(([key, name]) => [key, read(name)]),
  );
  const require = (text, marker, label) => {
    if (!containsMarker(text, marker))
      failures.push(`${label}: missing ${marker}`);
  };
  const forbid = (text, marker, label) => {
    if (containsMarker(text, marker))
      failures.push(`${label}: forbidden ${marker}`);
  };
  const body = (text, name) => {
    const result = rustFunctionBody(text, name);
    if (!result) failures.push(`missing function ${name}`);
    return result ?? "";
  };
  const ordered = (text, markers, label) => {
    let cursor = -1;
    const normalized = spaced(text);
    for (const marker of markers) {
      const next = normalized.indexOf(spaced(marker), cursor + 1);
      if (next < 0) {
        failures.push(`${label}: missing/out-of-order ${marker}`);
        break;
      }
      cursor = next;
    }
  };
  const load = body(source.plan, "load");
  for (const marker of [
    "MAX_QUANTITY_LEVELS: usize = 4096",
    "tenant_id.is_nil()",
    "actor_id.is_nil()",
    "variant_id.is_nil()",
    "require_current_snapshot(txn)",
    '"SHOW transaction_isolation"',
    "performance_schema.events_transactions_current AS trx",
    "INNER JOIN performance_schema.threads AS thread",
    "thread.THREAD_ID = trx.THREAD_ID",
    "thread.PROCESSLIST_ID = CONNECTION_ID()",
    "trx.STATE = 'ACTIVE'",
    "trx.END_EVENT_ID IS NULL",
    "trx.AUTOCOMMIT = 'NO'",
    'eq_ignore_ascii_case("read committed")',
    "query.lock_exclusive()",
    "inventory_item_write_query(txn, variant_id).limit(2)",
    "items.len() != 1",
    "items[0].id.is_nil()",
    "discovered.is_empty()",
    "MAX_QUANTITY_LEVELS + 1",
    "location_ids.iter().any(Uuid::is_nil)",
    "stock_location::Column::TenantId.eq(tenant_id)",
    "order_by_asc(stock_location::Column::Id)",
    "current_location.deleted_at.is_some()",
    "current_location.code != candidate.code",
    "before.checked_add(adjustment)",
    "i64::from(after) - i64::from(before)",
    "i32::try_from(target)",
    "i32::try_from(total)",
    "level.incoming_quantity < 0",
    "!ids.insert(level.id)",
    "!locations.insert(level.location_id)",
    "QuantityWriteBlocker::EventUnrepresentable",
  ])
    require(source.plan, marker, "bounded proved checked quantity kernel");
  ordered(
    load,
    [
      "product_variant::Column::TenantId.eq(tenant_id)",
      "product::Column::TenantId.eq(tenant_id)",
      "parent != Some(product_id)",
      "inventory_item_write_query",
      "level_query(item_id)",
      "find_default_location_in_tx",
      "let locations = current_read",
      "let levels = current_read",
      "Level::binding",
      "available_quantity",
      "before == after",
      "candidate.ok_or",
      "stocked_target",
      "validate_event_quantities",
    ],
    "identity/lock/current/noop/admission order",
  );
  for (const marker of [
    "saturating_",
    "@@transaction_isolation",
    "@@session.transaction_isolation",
    "-adjustment",
    "-input.adjustment",
    "ensure_inventory_state",
    "create_initial_records",
    "ensure_default_location",
    ".commit()",
    "::delete_many(",
    "metadata: Set",
    "index_revision",
    "projection_epoch",
  ])
    forbid(
      source.plan,
      marker,
      "no reconstruction/clamping/helper commit/Index state",
    );
  const apply = body(source.plan, "apply");
  ordered(
    apply,
    [
      "if let Some((level, stocked)) = self.target",
      "event.validate()",
      "inventory_level::Entity::update_many()",
      "updated.rows_affected != 1",
      "publish_root_in_tx",
    ],
    "no-op/preflight/exact-effect/publication",
  );
  for (const marker of [
    "Id.eq(level.id)",
    "InventoryItemId.eq(level.inventory_item_id)",
    "LocationId.eq(level.location_id)",
    "StockedQuantity.eq(level.stocked_quantity)",
    "ReservedQuantity.eq(level.reserved_quantity)",
    "IncomingQuantity.eq(level.incoming_quantity)",
    "DomainEvent::InventoryLow",
    "QuantityWriteError::Publication",
  ])
    require(apply, marker, "exact physical counter and atomic notification");
  for (const marker of [
    "col_expr(inventory_level::Column::ReservedQuantity",
    "col_expr(inventory_level::Column::IncomingQuantity",
    ".publish_in_tx(",
    "::publish_in_tx(",
    "sys_events",
    "event_bus.publish(",
  ])
    forbid(source.plan, marker, "canonical publisher and preserved counters");
  const native = body(source.native, "execute_quantity_write");
  ordered(
    native,
    [
      "self.db.begin()",
      "super::quantity::write_quantity",
      "txn.rollback()",
      "txn.commit()",
      "Ok(update)",
    ],
    "native completion/rollback/typed committed result",
  );
  require(body(
    source.native,
    "set_inventory_update",
  ), "Change::Set(quantity)", "native set delegation");
  require(body(
    source.native,
    "adjust_inventory_update",
  ), "Change::Adjust(input.adjustment)", "native adjust delegation");
  const bootstrap = body(source.bootstrap, "update_initial_quantity_in_tx");
  for (const marker of [
    "super::quantity::write_quantity",
    "txn, tenant_id, actor_id, variant_id",
    "Change::Set(quantity)",
  ])
    require(bootstrap, marker, "real Product owner hook");
  require(source.bootstrap, "txn: &sea_orm::DatabaseTransaction", "concrete caller transaction");
  for (const marker of ["Entity::", ".commit()", "return Ok(())"])
    forbid(bootstrap, marker, "no arbitrary first-level/silent hook");
  require(source.commands, /update_initial_quantity_in_tx\(\s*&txn,\s*tenant_id,\s*actor_id,\s*variant_id,\s*quantity,?\s*\)/, "real Product current context");
  ordered(
    body(source.commands, "update_variant"),
    [
      "BootstrapService::update_initial_quantity_in_tx",
      "DomainEvent::VariantUpdated",
      "self.get_variant_in(&txn, tenant_id, variant_id)",
      "record_product_operation_result(&result)",
      "txn.commit()",
    ],
    "Product response is read and sealed on the caller transaction",
  );
  for (const marker of [
    "BootstrapService::load_available_quantities(conn, &variant_ids)",
    "inventory_quantity: available_inventory",
  ])
    require(body(
      source.projection,
      "get_variant_in",
    ), marker, "Product owner response reader");
  for (const text of [source.bootstrap, source.visible]) {
    require(text, "super::quantity::available_by_variant", "shared exact Product/visible projection arithmetic");
    forbid(text, "saturating_", "no order-dependent projection saturation");
  }
  require(source.commandPort, "inventory_quantity_blocker(error).is_some()", "finite receipt classification");
  require(source.commandPort, "PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict", "unknown invariant/storage outcome remains nonterminal");
  for (const marker of [
    "InventoryQuantityBlocked",
    "QuantityWriteError::Publication(CoreError::Database(error))",
    "QuantityWriteBlocker::PublicationUnverified",
    "error.kind == kind && error.code == code",
  ])
    require(source.productError, marker, "finite owner error preservation");
  for (const adapter of adapters)
    require(read(
      adapter,
    ), "CommerceError::InventoryQuantityBlocked", `exhaustive adapter ${adapter}`);
  for (const text of [source.http, source.graphql])
    require(text, "inventory_quantity_public_failure", "HTTP/GraphQL safe code+kind copy");
  require(source.http, "CommerceError::InventoryQuantityBlocked", "native HTTP policy");
  require(source.public, "inventory_quantity_public_policy", "native Product public policy");
  require(source.transaction, "rollback_deferred_transactions", "existing Product rollback-before-receipt finalization");
  require(source.target, "mod inventory_quantity_writes;", "registered regression target");
  for (const text of [source.root, source.services])
    for (const marker of ["QuantityWriteError", "QuantityWriteBlocker"])
      require(text, marker, "owner public exports");

  let packet = {};
  try {
    packet = JSON.parse(source.packet);
  } catch {
    failures.push("invalid source packet");
  }
  if (
    packet.packet !== "inventory-quantity-writes-source" ||
    packet.status !== "source_only_uncompiled_unrun"
  )
    failures.push("source-only packet identity/status drift");
  for (const key of [
    "compiled",
    "tests_executed",
    "sql_executed",
    "runtime_verified",
    "baseline_initialized",
    "inventory_revisions_added",
    "generic_index_complete",
    "step8_complete",
  ])
    if (packet[key] !== false) failures.push(`must not promote ${key}`);
  if (
    packet.bounds?.levels !== 4096 ||
    packet.regressions?.rust_definitions !== 33 ||
    packet.regressions?.node_definitions !== 10
  )
    failures.push("bounds/definition inventory drift");
  let definitions = 0;
  for (const [name, tests] of Object.entries(packet.regressions?.rust ?? {})) {
    const text = read(name);
    for (const name of tests) {
      require(text, `fn ${name}(`, "registered Rust regression");
      definitions += 1;
    }
  }
  if (packet.regressions?.executed !== false)
    failures.push("must not promote regression execution");
  if (definitions !== 33) failures.push("missing Rust regression names");
  const nodeDefinitions = (source.companion.match(/^test\(/gm) ?? []).length;
  if (nodeDefinitions !== 10)
    failures.push("missing Node regression definitions");
  for (const [name, expected] of Object.entries(packet.preserved_hashes ?? {}))
    if (
      crypto.createHash("sha256").update(read(name)).digest("hex") !== expected
    )
      failures.push(`preserved contract/migration changed: ${name}`);
  if (Object.keys(packet.preserved_hashes ?? {}).length !== 14)
    failures.push("missing preserved event/migration hashes");
  for (const [key, text] of [
    ["inventory", source.registry],
    ["product", source.productRegistry],
  ])
    require(text, "quantity-writes-source.json", `${key} evidence discovery`);
  require(source.package, '"verify:inventory:quantity-writes"', "direct read-only guard alias");
  require(source.package, '"test:verify:inventory:quantity-writes"', "maintainer-only companion alias");
  return {
    failures,
    rust_definitions: definitions,
    node_definitions: nodeDefinitions,
    compiled: false,
    tests_executed: false,
    runtime_verified: false,
    generic_index_complete: false,
  };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const result = verifyInventoryQuantityWrites();
  if (result.failures.length) {
    result.failures.forEach((message) => console.error(`✖ ${message}`));
    process.exitCode = 1;
  } else
    console.log(
      "✔ Inventory quantity source contract; 33 Rust/10 Node definitions unrun; compiler, SQL, concurrency, revisions and Index remain pending",
    );
}
