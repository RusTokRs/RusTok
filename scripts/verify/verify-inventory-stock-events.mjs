#!/usr/bin/env node

// Read-only source assertions, not Rust, SQL, relay or regression execution.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { containsMarker, spaced } from "./lib/source-text.mjs";
import { rustFunctionBody } from "./verify-cart-priced-write-context.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
export const sourceFiles = {
  events: "crates/modules/rustok-inventory/src/stock_events.rs",
  inventory: "crates/modules/rustok-inventory/src/lib.rs",
  shipment: "crates/modules/rustok-inventory/src/shipment_stock.rs",
  scope: "crates/modules/rustok-inventory/src/shipment_stock/scope.rs",
  returns: "crates/modules/rustok-inventory/src/return_restock.rs",
  outbox: "crates/modules/rustok-outbox/src/transport.rs",
  bus: "crates/modules/rustok-outbox/src/transactional.rs",
  search: "crates/modules/rustok-search/src/ingestion.rs",
  searchModule: "crates/modules/rustok-search/src/module.rs",
  projector: "crates/modules/rustok-search/src/projector_core.rs",
  physical:
    "crates/modules/rustok-commerce/tests/shipment_stock_provenance/stock_events.rs",
  parent: "crates/modules/rustok-commerce/tests/shipment_stock_provenance.rs",
  target:
    "crates/modules/rustok-commerce/tests/order_inventory_reservation_test.rs",
  companion: "scripts/verify/verify-inventory-stock-events.test.mjs",
  evidence:
    "crates/modules/rustok-inventory/contracts/evidence/native-stock-outbox-source.json",
};
export const sourcePaths = Object.values(sourceFiles);

export function verifyInventoryStockEvents({ root = repository } = {}) {
  const failures = [],
    sources = {};
  for (const [key, file] of Object.entries(sourceFiles)) {
    try {
      sources[key] = fs.readFileSync(path.join(root, file), "utf8");
    } catch {
      failures.push(`missing source: ${file}`);
    }
  }
  const require = (source, marker, label) => {
    if (!containsMarker(source, marker))
      failures.push(`${label}: missing ${marker}`);
  };
  const forbid = (source, marker, label) => {
    if (containsMarker(source, marker))
      failures.push(`${label}: forbidden ${marker}`);
  };
  const before = (source, first, second, label) => {
    const text = spaced(source ?? "");
    const a = text.indexOf(spaced(first)),
      b = text.indexOf(spaced(second));
    if (a < 0 || b < 0 || a >= b)
      failures.push(`${label}: ${first} must precede ${second}`);
  };
  const body = (source, name) => {
    const result = rustFunctionBody(source ?? "", name);
    if (!result) failures.push(`missing function body: ${name}`);
    return result ?? "";
  };
  const implementation = sources.events?.split("#[cfg(test)]")[0] ?? "";
  const prepare = body(implementation, "prepare");
  const publish = body(implementation, "publish");
  const changes = body(implementation, "validate_changes");
  const quantities = body(implementation, "load_available_quantities");

  require(sources.inventory, "mod stock_events;", "private native module is registered");
  forbid(
    sources.inventory,
    "pub mod stock_events;",
    "no new public stock-event capability",
  );
  require(implementation, "const MAX_STOCK_EVENT_LEVELS: usize = 1024", "bounded changed-level batch");
  require(changes, "changes.len() > MAX_STOCK_EVENT_LEVELS", "bounded changed-level batch");
  require(changes, "!seen.insert(change.key())", "duplicate level identity refuses");
  require(changes, "change.old_stocked == change.new_stocked && change.old_reserved == change.new_reserved", "physical equality only");
  forbid(
    changes,
    "change.available_delta() == 0",
    "equal availability must still invalidate a physical change",
  );
  for (const marker of [
    "if changes.is_empty()",
    "return Ok(Self::default())",
    "validate_projected_quantities(",
  ])
    require(prepare, marker, "preparation is bounded and can represent the final batch");
  before(
    prepare,
    "if changes.is_empty()",
    "event_actor_id(",
    "no-movement avoids new event dependencies",
  );
  forbid(prepare, "publish_root_in_tx(", "preparation never publishes");
  for (const marker of [
    "if self.changes.is_empty()",
    "load_available_quantities(",
    "for change in self.changes.into_values()",
    "DomainEvent::InventoryUpdated {",
    "old_quantity:",
    "new_quantity:",
    "self.before",
    "after",
    "TransactionalEventBus::publish_root_in_tx(",
    "txn, self.tenant_id, self.actor_id, event",
    '"inventory.stock_event_persistence_unavailable"',
  ])
    require(publish, marker, "canonical same-transaction physical invalidation");
  before(
    publish,
    "load_available_quantities(",
    "publish_root_in_tx(",
    "actual after-source snapshot precedes outbox",
  );
  for (const marker of [
    "OutboxTransport",
    '"sys_events"',
    "event_bus.publish(",
    "quantity.max(",
    "saturating_",
  ])
    forbid(
      implementation,
      marker,
      "no raw writer, secondary pipeline or quantity clamping",
    );
  for (const marker of [
    "bindings.len() > MAX_STOCK_EVENT_LEVELS",
    "SUM(il.stocked_quantity)",
    "SUM(il.reserved_quantity)",
    "LEFT JOIN stock_locations l ON l.id = il.location_id",
    "l.tenant_id = {} THEN 0 ELSE 1",
    "WHERE il.inventory_item_id IN ({placeholders}) GROUP BY il.inventory_item_id",
    "foreign_locations != 0",
    "quantities.len() != bindings.len()",
    "representable_quantity(available)",
  ])
    require(quantities, marker, "bounded tenant-proved global quantity snapshots");
  const actor = body(implementation, "event_actor_id");
  require(actor, "Uuid::parse_str(&context.actor.id)", "current human actor identity");
  require(actor, ".filter(|id| !id.is_nil())", "current human actor identity");
  require(actor, "PortActorKind::Service | PortActorKind::System => Ok(None)", "no fabricated service human");
  require(body(
    implementation,
    "representable_quantity",
  ), ".filter(|quantity| *quantity >= 0)", "v1 quantities never become negative");

  const apply = body(sources.shipment, "apply");
  before(
    body(sources.scope, "lock_items"),
    "load_tenant_variant(",
    "products.insert(*variant_id, variant.product_id)",
    "retain already tenant-proved Product identity",
  );
  require(apply, "products.get(variant_id)", "use saved Product identity without fresh eligibility");
  before(
    apply,
    "StockEventPlan::prepare(",
    "if application == Application::Prepare",
    "preflight event representability before carrier",
  );
  const probe = spaced(apply).slice(
    spaced(apply).indexOf("if application == Application::Prepare"),
    spaced(apply).indexOf("for line in ordered_lines.into_values()"),
  );
  forbid(probe, ".publish(", "rolled-back probe never publishes");
  before(
    apply,
    "active.reserved_quantity = Set(reserved)",
    "stock_events.publish(txn, context, OPERATION).await?",
    "stock publication participates in F transaction",
  );
  const afterPublish =
    spaced(apply).split(
      "stock_events.publish(txn, context, OPERATION).await?",
    )[1] ?? "";
  require(afterPublish, "require_remaining_budget(context, started)", "final native budget gate includes outbox IO");
  const returns = body(sources.returns, "restock_returned_items");
  before(
    returns,
    "already_restocked.push(",
    "StockEventPlan::prepare(",
    "exact return replay has no event snapshot dependency",
  );
  before(
    returns,
    "1 => {",
    "let changes = additions",
    "only actually admitted new receipts create changes",
  );
  before(
    returns,
    "StockEventPlan::prepare(",
    "for (key, addition) in additions",
    "return final values checked before physical update",
  );
  before(
    returns,
    "stock_events.publish(&txn, &context, RESTOCK_OPERATION).await?",
    "txn.commit()",
    "return outbox commits with receipt and stock",
  );
  require(returns, "products.insert(*variant_id, variant.product_id)", "retain return tenant proof without repricing");

  for (const name of [
    "write_envelope_in_tx",
    "write_contract_envelope_in_tx",
  ]) {
    const write = body(sources.outbox, name);
    require(write, "let affected =", "unconditional publication requires a persisted row");
    require(write, "require_inserted_event(affected)", "unconditional publication requires a persisted row");
  }
  require(body(
    sources.outbox,
    "require_inserted_event",
  ), "if affected != 1", "zero-row insert cannot ACK owner mutation");
  const once = body(sources.outbox, "write_contract_envelope_once_in_tx");
  require(once, "OnConflict::column(entity::Column::Id)", "write-once exact replay remains distinct");
  require(once, "entity::Entity::find_by_id(envelope_id)", "write-once exact replay remains distinct");
  require(sources.bus, "pub async fn publish_root_in_tx", "existing canonical root API");
  require(sources.search, "DomainEvent::InventoryUpdated { product_id, .. }", "existing Search invalidation consumer");
  require(sources.search, ".upsert_product(envelope.tenant_id, *product_id)", "existing Search invalidation consumer");
  require(sources.searchModule, "registry.register(SearchIngestionHandler::with_all_projection_sources(", "Search listener is mounted");
  require(sources.projector, "SUM(il.stocked_quantity::bigint - il.reserved_quantity::bigint)", "Search refetches source rather than consuming deltas");
  require(sources.parent, '#[path = "shipment_stock_provenance/stock_events.rs"]', "physical regression module is registered");
  require(sources.parent, "mod stock_events;", "physical regression module is registered");
  require(sources.target, "mod shipment_stock_provenance;", "physical parent is in a test target");

  try {
    const record = JSON.parse(sources.evidence);
    if (
      record.runtime_executed !== false ||
      record.step8_complete !== false ||
      record.source_regressions.executed !== false
    )
      failures.push(
        "evidence must not promote unrun regression/runtime or step8 completion",
      );
    const rustNames = record.source_regressions.rust_cases;
    let rustCount = 0;
    for (const group of rustNames) {
      const key = Object.keys(sourceFiles).find(
        (key) => sourceFiles[key] === group.file,
      );
      for (const name of group.names) {
        require(sources[
          key
        ], `fn ${name}(`, "registered focused Rust regression source");
        rustCount++;
      }
    }
    if (
      rustCount !== record.source_regressions.rust_definition_count ||
      rustCount !== 16
    )
      failures.push(
        "focused Rust regression source count must remain exactly 16 unrun definitions",
      );
    const nodeCount = (sources.companion?.match(/^test\(/gm) ?? []).length;
    if (
      nodeCount !== record.source_regressions.node_definition_count ||
      nodeCount !== 10
    )
      failures.push(
        "Node mutation source count must remain exactly 10 unrun definitions",
      );
  } catch (error) {
    failures.push(`invalid source-only evidence: ${error.message}`);
  }
  return {
    status: failures.length ? "failed" : "passed_source_only",
    failures,
    runtime_executed: false,
    step8_complete: false,
  };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const result = verifyInventoryStockEvents();
  if (result.failures.length) {
    result.failures.forEach((failure) => console.error(`✖ ${failure}`));
    process.exitCode = 1;
  } else
    console.log(
      "✔ Native Inventory/outbox source contract; Rust, SQL, concurrency, relay delivery, regressions and step 8 remain unverified",
    );
}
