#!/usr/bin/env node

// Read-only source assertions. No owner code, tests, SQL, Cargo or runtime is executed here.
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
  service: "crates/modules/rustok-inventory/src/services/inventory.rs",
  plan: "crates/modules/rustok-inventory/src/services/inventory/reservation_release.rs",
  ports: "crates/modules/rustok-inventory/src/ports.rs",
  terminal: "crates/modules/rustok-inventory/src/reservation_adoption.rs",
  pure: "crates/modules/rustok-inventory/src/services/inventory/reservation_release/tests.rs",
  persistence:
    "crates/modules/rustok-commerce/tests/inventory_quantity_release.rs",
  target: "crates/modules/rustok-commerce/tests/inventory_service_test.rs",
  cargo: "crates/modules/rustok-commerce/Cargo.toml",
  ui: "crates/modules/rustok-inventory/admin/src/ui/leptos.rs",
  english: "crates/modules/rustok-inventory/admin/locales/en.ftl",
  russian: "crates/modules/rustok-inventory/admin/locales/ru.ftl",
  evidence:
    "crates/modules/rustok-inventory/contracts/evidence/quantity-release-source.json",
  registry:
    "crates/modules/rustok-inventory/contracts/inventory-fba-registry.json",
  matrix:
    "crates/modules/rustok-inventory/contracts/evidence/inventory-contract-test-static-matrix.json",
  companion: "scripts/verify/verify-inventory-quantity-release.test.mjs",
};
export const sourcePaths = Object.values(sourceFiles);

export function verifyInventoryQuantityRelease({ root = repository } = {}) {
  const failures = [];
  const sources = Object.fromEntries(
    Object.entries(sourceFiles).map(([name, file]) => [
      name,
      fs.readFileSync(path.join(root, file), "utf8"),
    ]),
  );
  const runtime = (source) =>
    source.split(
      /#\[cfg\(test\)\]\s*(?:#\[path\s*=\s*"[^"\n]+"\]\s*)?mod\s+tests\s*[;{]/,
    )[0];
  const service = runtime(sources.service),
    plan = runtime(sources.plan);
  const ports = runtime(sources.ports);
  const implementation = ports.slice(
    ports.indexOf("impl InventoryReservationPort for"),
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
    const result = rustFunctionBody(source, name);
    if (!result) failures.push(`missing function ${name}`);
    return result ?? "";
  };
  const before = (source, first, second, label) => {
    const text = spaced(source),
      a = text.indexOf(spaced(first)),
      b = text.indexOf(spaced(second));
    if (a < 0 || b < 0 || a >= b)
      failures.push(`${label}: ${first} must precede ${second}`);
  };

  const release = body(service, "release_reservation_quantity");
  const reserve = body(service, "reserve");
  for (const source of [release, reserve]) {
    require(source, "if quantity == 0", "read-only zero branch");
    before(
      source,
      "self.load_variant(&txn, tenant_id, variant_id)",
      "if quantity == 0",
      "tenant proof before noop",
    );
    forbid(
      source,
      "DomainEvent::InventoryUpdated",
      "no negative-backorder v1 substitution",
    );
    forbid(
      source,
      "StockEventPlan",
      "physical v1 representability is not reservation policy",
    );
  }
  before(
    release,
    "if quantity == 0",
    "inventory_item_write_query(&txn, variant_id)",
    "zero avoids owner write locks",
  );
  before(
    release,
    "SelectedQuantityRelease::select(",
    "let locations_query",
    "saved allocation before locations",
  );
  before(
    release,
    "let locations_query",
    "let levels_query",
    "location locks before exact levels",
  );
  before(
    release,
    "selected.bind_levels(",
    "plan.apply(&txn)",
    "complete plan before writes",
  );
  before(
    release,
    "plan.apply(&txn)",
    "let available_quantity",
    "unrepresentable result rolls back before commit",
  );
  before(
    release.slice(release.indexOf("plan.apply(&txn)")),
    "let available_quantity",
    "txn.commit()",
    "result before commit",
  );
  for (const marker of [
    "Column::TenantId.eq(tenant_id)",
    "Column::Id.is_in(location_ids.clone())",
    "locations.len() != location_ids.len()",
    "Column::InventoryItemId.eq(inventory_item.id)",
    "Column::LocationId.is_in(location_ids.clone())",
    "load_active_reservation_claims(",
  ])
    require(release, marker, "tenant/original location plan");
  for (const marker of [
    "async fn release_reservation_items",
    "async fn release_reserved_quantity_from_levels",
  ])
    forbid(service, marker, "independent FIFO reducers removed");

  const rows = body(service, "load_quantity_reservation_rows");
  for (const marker of [
    "Column::InventoryItemId.eq(inventory_item_id)",
    "Column::DeletedAt.is_null()",
    "Column::ExternalId.is_null()",
    "Column::LineItemId.is_null()",
    "order_by_asc(entities::reservation_item::Column::CreatedAt)",
    "order_by_asc(entities::reservation_item::Column::Id)",
    "(QUANTITY_RESERVATION_ROW_LIMIT + 1) as u64",
    ".lock_exclusive()",
  ])
    require(rows, marker, "bounded anonymous-pool probe");
  const inspect = body(plan, "inspect_anonymous_pool");
  for (const marker of [
    "QUANTITY_RESERVATION_ROW_LIMIT: usize = 4096",
    "rows.len() > QUANTITY_RESERVATION_ROW_LIMIT",
    "row.external_id.is_some() || row.line_item_id.is_some()",
    "legacy_inventory_service",
    "saved_variant != Some(variant_id)",
    "has_terminal_disposition(&row.metadata)",
    '"reservation_id"',
    '"checkout_operation_id"',
    '"cart_line_item_id"',
    '"order_line_item_id"',
    "row.quantity <= 0",
    "!seen.insert(row.id)",
    "rows.sort_by_key(|row| (row.created_at, row.id))",
  ])
    require(marker.startsWith("QUANTITY_")
      ? plan
      : inspect, marker, "positive anonymous provenance");
  for (const marker of [
    "inventory_disposition",
    "superseded_by",
    "released_at",
    "disposed_at",
    "superseded_at",
  ])
    require(body(
      sources.terminal,
      "has_terminal_disposition",
    ), `"${marker}"`, "shared terminal semantics");
  require(body(
    sources.terminal,
    "has_terminal_disposition",
  ), "!value.is_null()", "false/zero are terminal, not absence");
  before(
    reserve,
    "reservation_release::admit_new_reservation(",
    "let mut level_update",
    "no new stranded anonymous rows",
  );
  before(
    reserve,
    "checked_add(quantity)",
    "let mut level_update",
    "counter overflow before SQL",
  );
  require(reserve, ".exec_without_returning(&txn)", "count anonymous reservation persistence");
  require(reserve, "if inserted != 1", "suppressed anonymous insert cannot acknowledge success");
  require(body(
    plan,
    "admit_new_reservation",
  ), "rows.len() >= QUANTITY_RESERVATION_ROW_LIMIT", "new pool row bound");

  const claims = body(service, "load_active_reservation_claims");
  for (const marker of [
    "Column::InventoryItemId.eq(inventory_item_id)",
    "Column::DeletedAt.is_null()",
    "Column::LocationId.is_in(location_ids.iter().copied())",
    "Column::Quantity.lte(0)",
    "Expr::col(entities::reservation_item::Column::Quantity).sum()",
    "group_by(entities::reservation_item::Column::LocationId)",
    "having(sum.lte(i32::MAX))",
    'cast_as(sea_orm::sea_query::Alias::new("SIGNED"))',
    "into_tuple::<(Uuid, i64)>()",
  ])
    require(claims, marker, "all-consumer exact claim aggregate");
  require(claims, "query.lock_exclusive()", "MySQL claim coverage uses a current locking read");
  for (const marker of ["Column::ExternalId", "Column::LineItemId", ".limit("])
    forbid(claims, marker, "do not cap or exclude keyed claims");
  const bind = body(plan, "bind_levels");
  for (const marker of [
    "active_claims.len() != expected",
    "location.tenant_id != tenant_id",
    "level.inventory_item_id != self.inventory_item_id",
    "!seen.insert(level.location_id)",
    "current_claims < selected_pool_claims",
    "i64::from(level.reserved_quantity) < current_claims",
    ".checked_sub(released)",
    "location.deleted_at.is_some()",
    "level.stocked_quantity != 0",
    "new_reserved != 0",
    "level.incoming_quantity != 0",
  ])
    require(bind, marker, "exact original level and full claim protection");
  const apply = body(plan, "apply");
  require(plan, /async fn apply\(self, txn: &DatabaseTransaction\)/, "apply requires native owner transaction");
  for (const marker of [
    "Column::Quantity.eq(row.quantity)",
    "Column::LocationId.eq(row.location_id)",
    "Column::ReservedQuantity.eq(level.reserved_quantity)",
    "Column::StockedQuantity.eq(level.stocked_quantity)",
    "Column::IncomingQuantity.eq(level.incoming_quantity)",
    "Column::ExternalId.is_null()",
    "Column::LineItemId.is_null()",
  ])
    require(apply, marker, "conditional exact saved writes");
  if ((apply.match(/rows_affected\s*!=\s*1/g) ?? []).length !== 2)
    failures.push(
      "every reservation and level update must affect exactly one row",
    );
  forbid(apply, "Column::Metadata", "no destructive history rewrite");
  forbid(
    apply,
    "Column::StockedQuantity,",
    "release is not a physical stock movement",
  );

  const sharedLock = body(service, "inventory_item_write_query");
  require(sharedLock, "DbBackend::Sqlite", "explicit SQLite lock profile");
  require(sharedLock, ".lock_exclusive()", "shared native item row lock");
  require(body(
    service,
    "ensure_inventory_item",
  ), "inventory_item_write_query(conn, variant.id)", "native writes use shared item lock");
  require(body(
    ports,
    "load_inventory_item_for_update",
  ), "crate::services::inventory::inventory_item_write_query(conn, variant_id)", "identity/adoption use same item lock");
  for (const [name, parent, forward] of [
    ["reserve_inventory", "request.cart_id.is_some()", ".reserve(&context"],
    [
      "release_inventory_reservation",
      "request.order_id.is_some()",
      ".release_reservation_quantity(&context",
    ],
  ]) {
    const method = body(implementation, name);
    require(method, parent, "do not discard explicit parent selector");
    require(method, "request.line_item_id.is_some()", "do not discard explicit line selector");
    before(
      method,
      "require_anonymous_quantity_scope(",
      forward,
      "scope refusal before native lookup",
    );
  }
  require(body(
    ports,
    "require_anonymous_quantity_scope",
  ), '"inventory.reservation_identity_required"', "actionable scoped-request refusal");
  const availability = body(service, "checked_available_quantity");
  for (const marker of [
    "try_fold(0_i64",
    "i64::from(level.stocked_quantity)",
    "i64::from(level.reserved_quantity)",
    "i32::try_from(total)",
  ])
    require(availability, marker, "signed checked availability");
  for (const marker of ["saturating_", ".clamp(", ".max(0)", ".sum::<i32>()"])
    forbid(availability, marker, "no fabricated/clamped availability");
  require(body(
    service,
    "available_quantity",
  ), "(STOCK_LEVEL_READ_LIMIT + 1) as u64", "bounded level read");

  require(sources.service, '#[path = "inventory/reservation_release.rs"]', "private owner module registration");
  require(sources.plan, '#[path = "reservation_release/tests.rs"]', "pure regression registration");
  require(sources.target, '#[path = "inventory_quantity_release.rs"]', "persistence regression registration");
  require(sources.target, "mod quantity_release;", "persistence regression registration");
  require(sources.cargo, "autotests = false", "child fixtures are not standalone targets");
  require(sources.cargo, 'name = "inventory_service_test"', "canonical explicit integration target");
  require(sources.cargo, 'path = "tests/inventory_service_test.rs"', "canonical explicit integration target");
  require(sources.english, "inventory-action-releaseReservation = Release anonymous reservation", "honest operator scope");
  require(sources.russian, "inventory-action-releaseReservation = Освободить анонимный резерв", "localized operator scope");
  require(sources.ui, '"Release anonymous reservation"', "matching fallback copy");

  try {
    const evidence = JSON.parse(sources.evidence);
    const rustCount = [sources.pure, sources.persistence].reduce(
      (sum, source) =>
        sum + (source.match(/#\[(?:tokio::)?test\]/g) ?? []).length,
      0,
    );
    const nodeCount = (sources.companion.match(/^test\(/gm) ?? []).length;
    if (
      rustCount !== 27 ||
      rustCount !== evidence.source_regressions.rust_definition_count
    )
      failures.push(
        "Rust source definitions must remain exactly 27, uncompiled and unrun",
      );
    if (
      nodeCount !== 10 ||
      nodeCount !== evidence.source_regressions.node_definition_count
    )
      failures.push("Node source definitions must remain exactly 10, unrun");
    if (
      evidence.source_regressions.rust_executed !== 0 ||
      evidence.source_regressions.node_executed !== 0
    )
      failures.push("source-only evidence cannot promote regression execution");
    for (const flag of [
      "compiled",
      "sql_executed",
      "runtime_certified",
      "notification_delivery_certified",
      "step8_complete",
    ])
      if (evidence[flag] !== false)
        failures.push(`source-only evidence cannot certify ${flag}`);
    for (const cases of [
      JSON.parse(sources.registry).contract_tests?.cases,
      JSON.parse(sources.matrix).cases,
    ]) {
      if (!Array.isArray(cases))
        failures.push("quantity contract matrix missing cases");
      else
        for (const operation of [
          "reserve_inventory",
          "release_inventory_reservation",
        ])
          if (
            !cases
              .find((item) => item.operation === operation)
              ?.assertions.includes("anonymous_pool_only_no_identity_fallback")
          )
            failures.push(
              `${operation}: missing anonymous-pool matrix contract`,
            );
    }
  } catch (error) {
    failures.push(`invalid source-only evidence/matrix: ${error.message}`);
  }
  return {
    status: failures.length ? "failed" : "passed_source_only",
    failures,
    tests_executed: false,
    sql_executed: false,
    step8_complete: false,
  };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const result = verifyInventoryQuantityRelease();
  if (result.failures.length) {
    result.failures.forEach((failure) => console.error(`✖ ${failure}`));
    process.exitCode = 1;
  } else
    console.log(
      "✔ Anonymous quantity reservation source contract; tests, SQL, compilation, notifications and step 8 remain unverified",
    );
}
