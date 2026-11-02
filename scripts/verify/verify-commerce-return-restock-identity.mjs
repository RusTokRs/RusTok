#!/usr/bin/env node

// Read-only source guard. No Rust, SQL, providers or regression tests execute here.
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
  inventory: "crates/modules/rustok-inventory/src/return_restock.rs",
  terminal: "crates/modules/rustok-inventory/src/reservation_adoption.rs",
  commerce: "crates/modules/rustok-commerce/src/services/return_restock.rs",
  orchestration:
    "crates/modules/rustok-commerce/src/services/return_completion_orchestration.rs",
  order: "crates/modules/rustok-order/src/order_read.rs",
  query: "crates/modules/rustok-order/src/order_line_inventory.rs",
  service: "crates/modules/rustok-order/src/services/order.rs",
  exports: "crates/modules/rustok-order/src/lib.rs",
  evidence:
    "crates/modules/rustok-commerce/contracts/evidence/return-restock-identity-source.json",
  decision: "DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md",
  storage:
    "crates/modules/rustok-inventory/src/migrations/m20260713_000021_bound_reservation_metadata_and_inactive_locations.rs",
};
export const sourcePaths = Object.values(sourceFiles);

export function verifyCommerceReturnRestockIdentity({
  root = repository,
} = {}) {
  const failures = [];
  const sources = Object.fromEntries(
    Object.entries(sourceFiles).map(([key, file]) => [
      key,
      key === "exports"
        ? fs.readFileSync(path.join(root, file), "utf8")
        : fs
            .readFileSync(path.join(root, file), "utf8")
            .split(/#\[cfg\(test\)\]/)[0],
    ]),
  );
  const require = (source, marker, label) => {
    if (!containsMarker(source, marker))
      failures.push(`${label}: missing ${marker}`);
  };
  const forbid = (source, marker, label) => {
    if (containsMarker(source, marker))
      failures.push(`${label}: forbidden ${marker}`);
  };
  const body = (source, name) => {
    const value = rustFunctionBody(source, name);
    if (!value) failures.push(`missing function ${name}`);
    return value ?? "";
  };
  const before = (source, first, second, label) => {
    const text = spaced(source),
      a = text.indexOf(spaced(first)),
      b = text.indexOf(spaced(second));
    if (a < 0 || b < 0 || a >= b)
      failures.push(`${label}: ${first} must precede ${second}`);
  };

  const owner = body(sources.inventory, "restock_returned_items");
  require(sources.inventory, "context: PortContext", "canonical owner context");
  require(sources.inventory, "pub variant_id: Option<Uuid>", "explicit saved SKU binding");
  require(sources.inventory, "pub const MAX_RETURN_RESTOCK_ITEMS: usize = 1024", "bounded native batch");
  before(
    owner,
    "context.require_policy(PortCallPolicy::write())?",
    ".begin()",
    "admission before storage",
  );
  before(
    owner,
    "validate_request(lines)?",
    ".begin()",
    "whole batch preflight",
  );
  before(
    owner,
    "load_tenant_variant(",
    "load_inventory_item_for_update(",
    "tenant proof before item lock",
  );
  before(
    owner,
    "load_inventory_item_for_update(",
    "read_receipt(",
    "proved item locks before saved state",
  );
  before(
    owner,
    "load_inventory_item_for_update(",
    "reservation_item::Entity::find()",
    "tenant proof before reservation lookup",
  );
  for (const marker of [
    ".collect::<BTreeSet<_>>()",
    "items.insert(variant_id, item)",
    "let Some(variant_id) = line.variant_id else",
    "if existing.is_some()",
    "outcome.untracked.push(line.return_item_id)",
    "reservation_item::Column::InventoryItemId.eq(item.id)",
    "reservation_item::Column::LineItemId.eq(line.order_line_item_id)",
    ".limit(2)",
    "reservations.len() != 1",
    "matching.len() != 1",
    ".lock_exclusive()",
    "prove_location(",
    "let level_keys",
    "let mut levels",
    "movements.values()",
    "OnConflict::column(ledger::Column::ReturnItemId)",
    ".exec_without_returning(&txn)",
    "total.checked_add(movement.line.quantity)",
    "level.stocked_quantity.checked_add(addition)",
    "active.stocked_quantity = Set(stocked)",
    "txn.commit()",
  ])
    require(owner, marker, "Inventory identity/atomicity");
  if ((spaced(owner).match(/\.all\(&txn\)/g) ?? []).length !== 2)
    failures.push(
      "bounded ambiguity proof must read both candidate sets, never one row",
    );
  before(
    owner,
    "validate_replay(&saved",
    "reservation_item::Entity::find()",
    "exact replay before allocation selection",
  );
  before(
    owner,
    "let level_keys",
    "ledger::Entity::insert",
    "stable level locks before unique receipt admission",
  );
  before(
    owner,
    "ledger::Entity::insert",
    "active.stocked_quantity",
    "receipt fence before stock addition",
  );
  before(
    owner,
    "active.stocked_quantity",
    "txn.commit()",
    "single native atomic commit",
  );
  for (const marker of [
    ".order_by_asc(",
    "active.reserved_quantity =",
    "active.incoming_quantity =",
    "reservation_item::ActiveModel",
    "metadata = Set(",
  ])
    forbid(owner, marker, "no first-row choice or reservation mutation");
  if ((owner.match(/outcome\.untracked\.push/g) ?? []).length !== 1)
    failures.push("only explicit SKU-less binding may be untracked");

  const request = body(sources.inventory, "validate_request");
  for (const marker of [
    "lines.is_empty()",
    "lines.len() > MAX_RETURN_RESTOCK_ITEMS",
    "validate_non_nil_uuid(line.return_id",
    "validate_non_nil_uuid(line.return_item_id",
    "validate_non_nil_uuid(line.order_line_item_id",
    "validate_non_nil_uuid(variant_id",
    "line.quantity <= 0",
    "line.return_id != return_id",
    "!seen.insert(line.return_item_id)",
  ])
    require(request, marker, "bounded distinct single-return preflight");
  const receipt = body(sources.inventory, "read_receipt");
  require(receipt, "ledger::Column::TenantId.eq(tenant_id)", "tenant-scoped receipt reads");
  const replay = body(sources.inventory, "validate_replay");
  for (const marker of [
    "saved.tenant_id != tenant_id",
    "saved.return_item_id != line.return_item_id",
    "saved.return_id != line.return_id",
    "saved.order_line_item_id != line.order_line_item_id",
    "saved.inventory_item_id != item_id",
    "saved.quantity != line.quantity",
    "saved.location_id.is_nil()",
    "expected_location.is_some_and(",
  ])
    require(replay, marker, "complete ledger replay correlation");
  const location = body(sources.inventory, "prove_location");
  require(location, "stock_location::Column::TenantId.eq(tenant_id)", "saved location tenant proof");
  forbid(
    location,
    "DeletedAt.is_null()",
    "exact replay can prove an archived saved location",
  );
  const receiving = body(sources.inventory, "lock_receiving_locations");
  before(
    owner,
    "lock_receiving_locations(&txn, &context, tenant_id, &movements)",
    "let level_keys",
    "fresh receiving locations are proved before levels or receipts",
  );
  require(receiving, ".map(|movement| movement.location_id).collect::<BTreeSet<_>>()", "receiving location locks have stable distinct UUID order");
  require(receiving, "stock_location::Column::TenantId.eq(tenant_id)", "receiving location ownership is tenant-proved");
  require(receiving, "receiving_query.lock_exclusive()", "receiving location lock serializes concurrent archival");
  require(receiving, "DbBackend::Sqlite", "SQLite receiving locations use native transaction serialization");
  require(receiving, "if !location.is_active()", "archived receiving locations require reconciliation before writes");
  require(receiving, '"inventory.return_restock_location_archived"', "archived receiving refusal is actionable");
  require(receiving, "PortError::invariant_violation(", "archived receiving refusal is nonretryable reconciliation, not storage unavailability");
  forbid(
    receiving,
    "checked.contains",
    "replay ownership cache is not fresh write proof",
  );
  forbid(
    receiving,
    ".update(",
    "no implicit location reactivation or relocation",
  );
  require(sources.storage, "deleted stock locations cannot carry inventory quantities", "existing PostgreSQL inactive-location invariant");
  require(sources.storage, "active inventory quantities require an active stock location", "existing SQLite inactive-location invariant");
  const allocation = body(sources.inventory, "validate_allocation");
  for (const marker of [
    "reservation.inventory_item_id != item_id",
    "reservation.line_item_id != Some(line.order_line_item_id)",
    "has_terminal_disposition(&reservation.metadata)",
    "!reservation.metadata.is_object()",
    '"variant_id"',
    '"order_line_item_id"',
    '"inventory_item_id"',
    '"location_id"',
  ])
    require(allocation, marker, "retained allocation facts, not repair");
  for (const marker of [
    '"inventory_disposition"',
    '"superseded_by"',
    '"released_at"',
    '"disposed_at"',
    '"superseded_at"',
  ])
    require(body(
      sources.terminal,
      "has_terminal_disposition",
    ), marker, "one existing terminal-fact predicate");

  require(sources.order, /async fn read_order_line_inventory_bindings\(\s*&self,\s*context:\s*PortContext,\s*request:\s*ReadOrderLineInventoryBindingsRequest\s*,?\s*\)\s*->\s*Result<OrderLineInventoryBindingsSnapshot,\s*PortError>;/, "mandatory existing owner read capability");
  require(sources.order, "pub const MAX_ORDER_LINE_INVENTORY_BINDINGS: usize = 1024", "bounded owner projection");
  const bindingDto = sources.order.slice(
    sources.order.indexOf("pub struct OrderLineInventoryBinding {"),
    sources.order.indexOf("pub struct OrderLineInventoryBindingsSnapshot {"),
  );
  require(bindingDto, '#[serde(deserialize_with = "deserialize_saved_variant_id")]', "SKU-less null requires explicit field presence");
  require(bindingDto, "Option::<Uuid>::deserialize(deserializer)", "required nullable saved Variant decode");
  forbid(bindingDto, "#[serde(default)]", "missing SKU field is not SKU-less");
  const implementation = sources.order.slice(
    sources.order.indexOf("impl OrderReadPort for InProcessOrderReadPort"),
  );
  const read = body(implementation, "read_order_line_inventory_bindings");
  for (const marker of [
    "context.require_policy(PortCallPolicy::read())?",
    "parse_tenant_id(&context, OPERATION)?",
    "validate_inventory_binding_request(&request)?",
    "self.inner",
    ".read_order_line_inventory_bindings(tenant_id, request)",
    "map_owner_error(",
  ])
    require(read, marker, "admitted canonical Order implementation");
  before(
    read,
    "validate_inventory_binding_request(&request)?",
    "self.inner",
    "read batch admission before owner IO",
  );
  const validation = body(sources.order, "validate_inventory_binding_request");
  for (const marker of [
    "request.order_line_item_ids.is_empty()",
    "request.order_line_item_ids.len() > MAX_ORDER_LINE_INVENTORY_BINDINGS",
    "!seen.insert(*line_id)",
  ])
    require(validation, marker, "Order read identity bounds");
  for (const marker of [
    "ReadOrderLineInventoryBindingsRequest",
    "OrderLineInventoryBinding",
    "OrderLineInventoryBindingsSnapshot",
  ])
    require(sources.exports, marker, "public owner binding DTOs");
  const query = body(sources.query, "read_order_line_inventory_bindings");
  for (const marker of [
    ".inner_join(order::Entity)",
    "order::Column::TenantId.eq(tenant_id)",
    "order_line_item::Column::OrderId.eq(request.order_id)",
    "order_line_item::Column::Id.is_in(",
    ".select_only()",
    ".column(order_line_item::Column::VariantId)",
    ".column(order_line_item::Column::Quantity)",
    "MAX_ORDER_LINE_INVENTORY_BINDINGS + 1",
    "rows.len() != expected.len()",
    "received != expected",
  ])
    require(query, marker, "tenant-proved bounded price-free saved projection");
  for (const marker of [
    "rustok_product",
    "rustok_pricing",
    "UnitPrice",
    "TotalPrice",
    "calculate_price",
    ".get_order(",
  ])
    forbid(sources.query, marker, "no current Catalog/Pricing refresh");
  require(sources.service, /crate::order_line_inventory::read_order_line_inventory_bindings\(\s*&self\.db,\s*tenant_id,\s*request\s*,?\s*\)/, "owner service delegation");

  const prepare = body(sources.commerce, "prepare_restock_plan");
  const plan = body(sources.commerce, "plan_restock_lines");
  for (const marker of [
    "order_reads: &dyn OrderReadPort",
    "order_reads.read_order_line_inventory_bindings(",
    "tokio::time::timeout(",
    "if line_ids.is_empty()",
  ])
    require(sources.commerce, marker, "selected provider and explicit no-selection path");
  before(
    prepare,
    "call_budget(&context, PortCallPolicy::read())",
    "order_reads.read_order_line_inventory_bindings(",
    "positive budget before ready provider poll",
  );
  for (const marker of [
    "snapshot.tenant_id != tenant_id",
    "snapshot.order_id != order_return.order_id",
    "snapshot.items.len() != expected.len()",
    "bindings.insert(binding.order_line_item_id, binding).is_some()",
    "binding.variant_id.is_some_and(",
    "item.quantity > binding.quantity",
    "variant_id: binding.variant_id",
  ])
    require(plan, marker, "complete saved SKU/quantity response correlation");
  for (const marker of [
    "reservation_item::",
    "inventory_item::",
    "inventory_level::",
    "order_line_item::",
    "in_process_order_read_port(",
    ".read_order_projection(",
    "release_inventory",
    "generate_id(",
  ])
    forbid(
      sources.commerce,
      marker,
      "no foreign rows, fallback, reprice or compensation",
    );
  const selected = body(sources.commerce, "selected_items");
  before(
    selected,
    "if explicit_no_goods(decision)",
    "return_item_facts(tenant_id, order_return)?",
    "explicit no-goods decision is not a stock-quota or availability dependency",
  );
  const completed = body(sources.commerce, "validate_completed");
  for (const marker of [
    'completed.status != "completed"',
    "completed.id != self.return_id",
    "completed.order_id != self.order_id",
    "if let Some(expected) = &self.return_items",
    "return_item_facts(self.tenant_id, completed)? != *expected",
  ])
    require(completed, marker, "frozen plan must survive owner completion");
  const apply = body(sources.commerce, "apply_restock_plan");
  before(
    apply,
    "plan.validate_completed(completed)?",
    "restock_returned_items(db, context, &plan.lines)",
    "completion proof before stock effect",
  );
  before(
    apply,
    "call_budget(&context, PortCallPolicy::write())",
    "restock_returned_items(db, context, &plan.lines)",
    "write admission before stock future",
  );
  require(apply, "validate_restock_outcome(&plan.lines, &outcome)?", "complete Inventory outcome correlation");
  const outcome = body(sources.commerce, "validate_restock_outcome");
  for (const marker of [
    "total != lines.len()",
    "total > MAX_RETURN_RESTOCK_ITEMS",
    "*quantity != line.quantity",
    "!seen.insert(*id)",
    "line.variant_id.is_none()",
    "line.variant_id.is_some()",
    "seen.len() != lines.len()",
  ])
    require(outcome, marker, "no missing duplicate wrong-quantity or wrong-tracking outcome");
  const budget = body(sources.commerce, "call_budget");
  require(budget, "context.require_policy(policy)?", "canonical positive-deadline policy");
  require(budget, ".min(MAX_RESTOCK_BUDGET)", "bounded selected-call budget");
  const execute = body(sources.orchestration, "execute_claimed");
  require(execute, "self.order_read_port.as_ref()", "reuse the selected Arc");
  before(
    execute,
    "prepare_restock_plan(",
    ".resolve_refund(",
    "bind plan before resolution effects",
  );
  before(
    execute,
    "prepare_restock_plan(",
    ".complete_owner_return(",
    "bind plan before owner completion",
  );
  before(
    execute,
    "self.restock_returned_goods(",
    ".checkpoint(",
    "restock before separate Commerce checkpoint",
  );
  require(sources.orchestration, 'format!("return-completion:{operation_id}:restock")', "same admitted mutation namespace");
  const disposition = body(sources.orchestration, "port_failure_disposition");
  require(disposition, "_ if error.retryable => FailureDisposition::Retryable", "original positive retry advice");
  require(disposition, "_ => FailureDisposition::Reconciliation", "false advice parks same identity");
  const failure = body(sources.orchestration, "record_failure");
  require(failure, "bounded_failure_note(error)", "bounded journal diagnostic projection");
  require(body(
    sources.orchestration,
    "bounded_failure_note",
  ), '"inventory.return_restock_location_archived"', "archived receiving refusal retains its bounded journal code");
  forbid(failure, "error.to_string()", "no arbitrary owner payload in journal");

  const evidence = JSON.parse(sources.evidence);
  if (
    evidence.status !== "source_implemented_runtime_unvalidated" ||
    evidence.step8_complete !== false ||
    evidence.runtime_executed !== false
  )
    failures.push("evidence must retain source-only and open step-8 status");
  for (const key of [
    "archived_saved_location_exact_replay_allowed",
    "archived_receiving_location_refuses",
    "sorted_receiving_location_locks_before_levels_and_receipts",
  ])
    if (evidence.contract?.[key] !== true)
      failures.push(
        `contract.${key} must retain the archived receiving-location correction`,
      );
  for (const key of [
    "rust_tests",
    "mjs_regressions",
    "rust_compilation",
    "sql_or_lock_execution",
    "remote_convergence",
    "shipment_cancel_reopen",
    "complete_outbox_index_invalidation",
  ])
    if (evidence.unverified?.[key] !== true)
      failures.push(`unverified.${key} must remain true`);
  require(sources.decision, "Return-restock identity amendment (2026-10-31)", "accepted bounded owner amendment");
  return {
    status: failures.length ? "failed" : "passed",
    scope: "read-only source contract, not tests or runtime certification",
    failures,
  };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const result = verifyCommerceReturnRestockIdentity({
    root: process.env.RUSTOK_VERIFY_REPO_ROOT?.trim() || repository,
  });
  if (result.failures.length) {
    console.error(result.failures.join("\n"));
    process.exitCode = 1;
  } else {
    console.log(
      "✔ Return-restock identity source contract; Rust/SQL/concurrency/regression execution and step-8 lifecycle completion remain unverified",
    );
  }
}
