#!/usr/bin/env node
// Read-only source contract: never executes Rust, SQL, carriers or regression callbacks.
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
  port: "crates/modules/rustok-fulfillment/src/shipment_stock.rs",
  fulfillment: "crates/modules/rustok-fulfillment/src/services/fulfillment.rs",
  stock: "crates/modules/rustok-fulfillment/src/services/fulfillment/stock.rs",
  admin: "crates/modules/rustok-fulfillment/src/admin_command.rs",
  recovery: "crates/modules/rustok-fulfillment/src/admin_command/recovery.rs",
  createAdmin: "crates/modules/rustok-fulfillment/src/admin_create_command.rs",
  createBridge:
    "crates/modules/rustok-commerce/src/services/journaled_create_label_provider.rs",
  journal:
    "crates/modules/rustok-fulfillment/src/services/provider_operation.rs",
  journalRecovery:
    "crates/modules/rustok-fulfillment/src/services/provider_operation_recovery.rs",
  admissionMigration:
    "crates/modules/rustok-fulfillment/src/migrations/m20261101_000121_enforce_provider_effect_admission.rs",
  fulfillmentRegistry:
    "crates/modules/rustok-fulfillment/src/migrations/mod.rs",
  fulfillmentSupport: "crates/modules/rustok-fulfillment/tests/support.rs",
  journalFixture:
    "crates/modules/rustok-fulfillment/tests/provider_operation_journal_test.rs",
  http: "crates/modules/rustok-commerce/src/controllers/admin/fulfillments_owner_commands.rs",
  httpMount: "crates/modules/rustok-commerce/src/controllers/admin/mod.rs",
  package: "package.json",
  inventory: "crates/modules/rustok-inventory/src/shipment_stock.rs",
  scope: "crates/modules/rustok-inventory/src/shipment_stock/scope.rs",
  proof: "crates/modules/rustok-inventory/src/shipment_stock/proof.rs",
  migration:
    "crates/modules/rustok-inventory/src/migrations/m20261101_000025_create_shipment_stock_proofs.rs",
  registry: "crates/modules/rustok-inventory/src/migrations/mod.rs",
  returns: "crates/modules/rustok-inventory/src/return_restock.rs",
  adapter: "crates/modules/rustok-commerce/src/services/shipment_stock.rs",
  bridge:
    "crates/modules/rustok-commerce/src/services/journaled_fulfillment_orchestration.rs",
  reconciliation:
    "crates/modules/rustok-commerce/src/services/fulfillment_reconciliation.rs",
  cancellation:
    "crates/modules/rustok-commerce/src/services/order_cancellation.rs",
  graphql:
    "crates/modules/rustok-commerce/src/graphql/mutations/provider_operations.rs",
  support: "crates/modules/rustok-commerce/tests/support/mod.rs",
  adoptionParent:
    "crates/modules/rustok-commerce/tests/inventory_service_test.rs",
  decision: "DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md",
  evidence:
    "crates/modules/rustok-commerce/contracts/evidence/shipment-stock-provenance-source.json",
};
export const sourcePaths = Object.values(sourceFiles);

export function verifyCommerceShipmentStockProvenance({
  root = repository,
} = {}) {
  const failures = [];
  const rawSources = Object.fromEntries(
    Object.entries(sourceFiles).map(([key, file]) => [
      key,
      fs.readFileSync(path.join(root, file), "utf8"),
    ]),
  );
  const sources = Object.fromEntries(
    Object.entries(rawSources).map(([key, value]) => [
      key,
      value.split(/#\[cfg\(test\)\]/)[0],
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

  for (const marker of [
    "async fn prepare(",
    "async fn apply(",
    "UnconfiguredShipmentStockPort",
    "NoopShipmentStockPort",
    "MAX_SHIPMENT_STOCK_ITEMS: usize = 1024",
  ])
    require(sources.port, marker, "consumer-owned bounded protocol");
  const prepare = body(sources.stock, "prepare_shipment_stock_with_status");
  before(
    prepare,
    "let scope = stock_scope(",
    ".shipment_stock",
    "owner facts before selected preparation",
  );
  require(prepare, "requires_stock_preparation(&scope)", "unknown header scope is not no-stock");
  forbid(prepare, ".begin()", "peer I/O outside F write transaction");
  require(sources.stock, "scope.action == ShipmentStockAction::Ship && scope.items.is_empty()", "explicit opt-out for header-only fresh ship");
  const local = body(sources.stock, "apply_prepared_stock");
  before(
    local,
    "current != prepared.scope",
    "plan.apply(",
    "locked complete correlation",
  );
  before(
    local,
    ".remaining_context()",
    "plan.apply(",
    "original budget before local stock writes",
  );
  require(local, "requires_stock_preparation(&current)", "same preparation/application classification");
  const execute = body(sources.admin, "execute_provider_operation");
  before(
    execute,
    "prepare_shipment_stock_with_status(",
    ".claim_execution_with_stock_scope(",
    "selected preparation before claim",
  );
  before(
    execute,
    ".claim_execution_with_stock_scope(",
    ".remaining_context()",
    "decreasing post-claim budget",
  );
  before(
    execute,
    ".remaining_context()",
    ".execute_ship(",
    "budget before carrier",
  );
  require(sources.journal, ".filter(provider_operation::Column::Status.is_in([PROVIDER_OPERATION_PENDING]))", "claim only from provably uninvoked state");
  require(sources.journal, "PROVIDER_OPERATION_ERROR", "legacy error outcome fence");
  require(sources.journal, ".correlate_shipment_stock_scope(", "native-only claim correlation");
  const claim = body(sources.journal, "claim_execution_with_stock_scope");
  const release = body(sources.journal, "release_uninvoked_claim");
  for (const marker of [
    "ProviderResult",
    "ProviderReference",
    "ProviderCompletedAt",
    "CommittedAt",
  ]) {
    const predicate = `.filter(provider_operation::Column::${marker}.is_null())`;
    require(claim, predicate, "claim excludes every retained outcome marker");
    require(release, predicate, "private release excludes every retained outcome marker");
  }
  before(
    claim,
    "UPDATE fulfillment_provider_operations SET updated_at = updated_at WHERE 0",
    "let parent_query",
    "SQLite writer fence before transaction-local reads",
  );
  require(body(
    sources.journal,
    "ensure_transition",
  ), "PROVIDER_OPERATION_EXECUTING => from == PROVIDER_OPERATION_PENDING", "legacy ERROR is not permission to execute");
  const unresolved = body(sources.journal, "unresolved_provider_effects");
  require(unresolved, "Column::Status.is_not_in([PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_COMMITTED])", "all ambiguous states fence progress");
  for (const marker of [
    "ProviderResult.is_not_null()",
    "ProviderReference.is_not_null()",
    "ProviderCompletedAt.is_not_null()",
    "CommittedAt.is_not_null()",
    "ProviderResult.is_null()",
    "ProviderCompletedAt.is_null()",
    "CommittedAt.is_null()",
  ])
    require(unresolved, `Column::${marker}`, "malformed Pending and incomplete Committed cannot hide effects");
  require(claim, ".filter(unresolved_provider_effects())", "one owner admission predicate");
  require(body(
    sources.stock,
    "assert_progress_not_fenced",
  ), ".filter(super::super::provider_operation::unresolved_provider_effects())", "same owner predicate fences local progress");
  const noEffect = body(sources.journalRecovery, "resolve_unknown_as_failed");
  require(noEffect, "Expr::value(PROVIDER_OPERATION_PENDING)", "explicit no-effect recovery retains identity and requeues Pending");
  for (const marker of [
    "PROVIDER_OPERATION_RECONCILIATION_REQUIRED",
    "PROVIDER_OPERATION_ERROR",
    "Column::TenantId.eq(tenant_id)",
    "Column::Id.eq(operation_id)",
    "Column::ProviderResult.is_null()",
    "Column::ProviderReference.is_null()",
    "Column::CommittedAt.is_null()",
  ])
    require(noEffect, marker, "operator no-effect cannot erase evidence or cross scope");
  for (const marker of [
    "Column::IdempotencyKey,",
    "Column::RequestPayload,",
    "Column::ProviderReference,",
    "Column::ProviderResult,",
  ])
    forbid(
      noEffect,
      marker,
      "no-effect resolution cannot rotate identity or erase outcomes",
    );
  const successResolution = body(
    sources.journalRecovery,
    "resolve_unknown_as_succeeded",
  );
  for (const marker of [
    "operator resolution contradicts the retained provider reference",
    "let reference_match = match existing.provider_reference",
    ".filter(reference_match)",
    "Some(saved) if saved == &provider_result",
    "Expr::col(provider_operation::Column::ProviderResult).into()",
    "Expr::col(provider_operation::Column::ProviderCompletedAt).into()",
    ".filter(result_match)",
    ".filter(provider_operation::Column::CommittedAt.is_null())",
  ])
    require(successResolution, marker, "success resolution preserves and CASes observed outcome evidence");
  const worklist = body(
    sources.journalRecovery,
    "list_reconciliation_required",
  );
  for (const marker of [
    "unresolved_provider_effects()",
    "Column::Status.ne(PROVIDER_OPERATION_EXECUTING)",
    ".order_by_asc(provider_operation::Column::UpdatedAt)",
    ".order_by_asc(provider_operation::Column::Id)",
  ])
    require(worklist, marker, "bounded unresolved worklist keeps explicit stale-execution quarantine");
  require(body(
    sources.journalRecovery,
    "quarantine_stale_executing",
  ), "COALESCE(provider_completed_at, CURRENT_TIMESTAMP)", "stale execution quarantine preserves an already saved completion timestamp");
  const admissionName = "m20261101_000121_enforce_provider_effect_admission";
  require(sources.fulfillmentRegistry, `${admissionName}::Migration`, "forward-only admission migration registered");
  require(sources.fulfillmentRegistry, `"${admissionName}", vec!["m20261003_000120_create_checkout_identity_anchor"]`, "admission rollout follows canonical Fulfillment schema");
  for (const marker of [
    "OLD.status = 'pending' AND NEW.status = 'executing'",
    "OLD.provider_completed_at IS NULL AND OLD.committed_at IS NULL",
    "NEW.status = 'pending'",
    "NEW.provider_reference IS DISTINCT FROM OLD.provider_reference",
    "NEW.provider_reference IS NOT OLD.provider_reference",
    "forward-only",
    "DatabaseBackend::Postgres",
    "DatabaseBackend::Sqlite",
  ])
    require(sources.admissionMigration, marker, "forward-only SQL admission/evidence guard");
  forbid(
    sources.admissionMigration,
    /OLD\.status\s*=\s*'provider_error'\s+AND\s+NEW\.status\s*=\s*'executing'/,
    "no SQL legacy ERROR execution shortcut",
  );
  for (const key of [
    "support",
    "fulfillmentSupport",
    "journalFixture",
    "journalRecovery",
  ])
    require(rawSources[
      key
    ], admissionName, "raw fixtures install the current forward-only admission guard");
  const fixtureGuard = body(
    sources.journalFixture,
    "ensure_provider_journal_guards",
  );
  before(
    fixtureGuard,
    ".take(3)",
    admissionName,
    "historical guard fixture reinstalls current admission last",
  );

  const recovery = body(sources.recovery, "retry_local_persistence_impl");
  before(
    recovery,
    "context.require_policy(PortCallPolicy::read())?",
    ".operation_journal",
    "current budget before recovery read",
  );
  before(
    recovery,
    "operation.idempotency_key.clone()",
    "require_write_admission(",
    "literal saved identity before recovery admission",
  );
  require(recovery, "caller_metadata", "private request descriptor is not caller metadata");
  require(recovery, "Some(Value::Null) => None", "saved null differs from empty selection");
  before(
    recovery,
    "let started = std::time::Instant::now()",
    ".operation_journal",
    "recovery elapsed budget starts before journal read",
  );
  before(
    recovery,
    "remaining_stock_context(&preserved_context, started)",
    "require_write_admission(",
    "local recovery does not refresh the current operator budget",
  );
  for (const name of [
    "ship_fulfillment_with_provider_result",
    "cancel_fulfillment_with_provider_result",
  ]) {
    const wrapper = body(sources.fulfillment, name);
    before(
      wrapper,
      "let started = std::time::Instant::now()",
      "validate_provider_stock_context(",
      "native recovery budget includes journal validation",
    );
    before(
      wrapper,
      "remaining_stock_context(&context, started)",
      ".prepare_shipment_stock(",
      "native recovery budget includes journal replay reads",
    );
  }

  for (const marker of [
    "provider_registry",
    "execute_provider_operation(",
    "PortActor::system",
    "PortContext::new(",
  ])
    forbid(
      sources.recovery,
      marker,
      "local recovery cannot reconstruct context or invoke carrier",
    );
  for (const marker of [
    "provider_operation::",
    "request_payload",
    "provider_result",
  ])
    forbid(
      sources.reconciliation,
      marker,
      "Commerce delegates private recovery to selected F",
    );
  require(sources.bridge, "self.selected_commands.clone().unwrap_or_else(", "selected F wins over native factory");

  const native = body(sources.inventory, "apply");
  before(
    native,
    "scope::lock_lines(",
    "scope::lock_items(",
    "line serialization before SKU item work",
  );
  before(
    native,
    "scope::lock_items(",
    "scope::lock_allocations(",
    "all tenant-proved items before tenantless rows",
  );
  before(
    native,
    "if application == Application::Prepare",
    "active.quantity = Set(",
    "preparation stops before reservation writes",
  );
  const early = body(sources.inventory, "validate_shipment_stock_preparation");
  require(early, "txn.rollback().await", "no durable preparation receipt or stock");
  require(native, "insert_new_proof(", "global proof key is probed before carrier");
  require(sources.inventory, "if affected != 1", "global proof-key collision refuses");
  require(sources.inventory, "OnConflict::column(proof::Column::FulfillmentItemId)", "unique proof primary-key fence");
  for (const marker of [
    "saved.variant_id != line.variant_id",
    "saved.fulfillment_quantity != line.fulfillment_quantity",
    "saved.shipped_quantity != line.shipped_quantity",
    "saved.restored_quantity == undelivered",
    "has_terminal_disposition(&allocation.metadata)",
    ".limit(2)",
    "if !location.is_active()",
    "pg_advisory_xact_lock",
    "shipment_stock_backend_unsupported",
  ])
    require(sources.scope, marker, "complete native provenance, allocation and backend checks");
  const itemLocks = body(sources.scope, "lock_items");
  before(
    itemLocks,
    "load_tenant_variant(",
    "load_inventory_item_for_update(",
    "prove the whole Variant batch first",
  );
  require(sources.adapter, ".read_order_line_inventory_bindings(", "selected price-free saved facts");
  forbid(
    sources.adapter,
    "in_process_order_read_port(",
    "no embedded selected O fallback",
  );
  require(sources.adapter, "scope.items.is_empty()", "native adapter refuses missing typed shipment facts");
  require(sources.inventory, "assert_no_returned_stock(", "returned-goods restoration/reconsumption exclusion");
  const restock = body(sources.returns, "restock_returned_items");
  before(
    restock,
    "already_restocked.push(",
    "assert_no_restored_shipment_stock(",
    "exact return replay precedes new shipment dependency",
  );
  require(sources.proof, 'table_name = "inventory_shipment_stock_proofs"', "I-owned additive proof");
  require(sources.migration, "rollback cannot discard nonempty", "no destructive provenance rollback");
  forbid(
    sources.migration,
    "inventory_return_restocks",
    "existing return receipt is not a shipment table",
  );
  require(sources.registry, "m20261101_000025_create_shipment_stock_proofs::Migration", "canonical migration registration");
  require(sources.support, "m20261101_000025_create_shipment_stock_proofs", "canonical fixture schema");
  require(sources.adoptionParent, "support::ensure_commerce_schema(&db).await", "adoption target installs the new native schema");

  for (const marker of [
    "order_cancellation.fulfillment_reads_missing",
    "order_cancellation.fulfillment_commands_missing",
    "expected_total.get_or_insert(response.total)",
    "response.total != total",
    "remaining.min(PAGE_SIZE)",
    "!child_ids.insert(line.id)",
    "line.order_line_item_id.is_nil()",
    ".cancel_pending_fulfillment(",
  ])
    require(sources.cancellation, marker, "bounded complete selected cancellation");
  forbid(
    body(sources.cancellation, "read_fulfillments"),
    "in_process_fulfillment_read_port",
    "incomplete selection cannot fall back to native reads",
  );
  for (const operation of ["ship", "deliver", "reopen", "reship", "cancel"]) {
    require(body(
      sources.graphql,
      `${operation}_fulfillment`,
    ), `\"${operation}\", idempotency_key`, "five caller-owned GraphQL mutation identities");
  }
  forbid(
    body(sources.graphql, "fulfillment_command_context"),
    "graphql-fulfillment:{fulfillment_id}:{operation}",
    "no colliding per-resource GraphQL identity",
  );

  require(sources.httpMount, '#[path = "fulfillments_owner_commands.rs"] pub mod fulfillments;', "actual mounted HTTP owner-command adapter");
  forbid(
    sources.http,
    /\bFulfillmentOrchestrationService::new\s*\(/,
    "no old concrete orchestration on the mounted lifecycle path",
  );
  for (const marker of [
    "auth.port_actor()",
    "request_context.correlation_id.clone()",
    "auth.scopes.iter().cloned()",
    "value.is_empty() || value.len() > 191",
  ])
    require(sources.http, marker, "canonical principal, request context and bounded literal HTTP key");
  const create = body(sources.fulfillment, "create_fulfillment_in_txn");
  before(
    create,
    "validate_create_fulfillment_input(",
    ".insert_fulfillment_in_txn(",
    "bounded creation refuses before parent persistence",
  );
  require(body(
    sources.fulfillment,
    "validate_create_fulfillment_input",
  ), "validate_fulfillment_items(", "creation validates the complete item batch before insertion");
  require(body(
    sources.fulfillment,
    "validate_fulfillment_items",
  ), "items.len() > crate::MAX_SHIPMENT_STOCK_ITEMS", "create cannot exceed the native complete projection bound");
  for (const marker of [
    ".checked_mul(crate::MAX_SHIPMENT_STOCK_ITEMS)",
    "limit.checked_add(1)",
    "owner_items.len() == crate::MAX_SHIPMENT_STOCK_ITEMS",
  ])
    require(sources.fulfillment, marker, "checked aggregate probe and complete per-parent item bound");
  const scripts = JSON.parse(sources.package).scripts;
  if (
    scripts["verify:commerce:shipment-stock-provenance"] !==
      "node scripts/verify/verify-commerce-shipment-stock-provenance.mjs" ||
    scripts["test:verify:commerce:shipment-stock-provenance"] !==
      "node scripts/verify/verify-commerce-shipment-stock-provenance.test.mjs"
  )
    failures.push(
      "shipment guard and unrun companion must have separate package commands",
    );
  require(sources.decision, "Shipment identity and native movement provenance amendment (2026-11-01)", "owning ADR amendment");
  // Targeted review: distinguish intent/replay from terminal projections and preserve facts.
  const nativeAdmin = sources.admin.slice(
    sources.admin.indexOf(
      "impl FulfillmentAdminCommandPort for InProcessFulfillmentAdminCommandPort",
    ),
  );
  for (const name of ["reship_fulfillment", "cancel_fulfillment_command"]) {
    const owner = body(nativeAdmin, name);
    forbid(
      owner,
      "return Ok(current)",
      "terminal status cannot authorize key replay",
    );
    require(owner, ".execute_provider_operation(", "journal authorizes key and payload before replay");
    require(owner, "remaining_stock_context(&context, started)", "root owner budget includes initial projection and provider lookup");
  }
  const nativeCreate = body(
    sources.createAdmin.slice(
      sources.createAdmin.indexOf(
        "impl FulfillmentAdminCreateCommandPort for InProcessFulfillmentAdminCreateCommandPort",
      ),
    ),
    "create_fulfillment",
  );
  forbid(
    nativeCreate,
    "PROVIDER_OPERATION_SUCCEEDED",
    "create success checkpoint is not native completion receipt",
  );
  require(nativeCreate, "operation.status == PROVIDER_OPERATION_COMMITTED", "create success checkpoint is not native completion receipt");
  const reship = body(nativeAdmin, "reship_fulfillment");
  require(reship, "Some((ShipmentStockAction::Reconsume, None))", "Delivered reship freezes a movement-free native scope");
  require(reship, 'Some("delivered")', "Delivered reship freezes a movement-free native scope");
  require(reship, ".reship_fulfillment_with_prepared_provider_result(", "reship retains scope and budget through local application");
  const pin = body(sources.admin, "provider_id_for_fulfillment");
  before(
    pin,
    ".find_fulfillment_key(",
    "match fulfillment.shipping_option_id",
    "retained provider identity precedes mutable provider selection",
  );
  const identity = body(sources.journal, "find_fulfillment_key_on_connection");
  for (const marker of [
    "Column::TenantId.eq(tenant_id)",
    "Column::FulfillmentId.eq(fulfillment_id)",
    "Column::IdempotencyKey.eq(key)",
    ".limit(2)",
    "provider_identity_ambiguous()",
  ])
    require(identity, marker, "bounded original-key lookup refuses historical provider ambiguity");
  forbid(
    identity,
    "Column::Status",
    "original-key collisions include Pending and Committed",
  );
  const admission = body(sources.journal, "begin_on_connection");
  before(
    admission,
    ".lock_exclusive()",
    "find_fulfillment_key_on_connection(",
    "parent-serialized cross-provider key admission",
  );
  require(body(
    sources.journal,
    "begin",
  ), "let txn = self.db.begin().await?", "journal begin owns its admission transaction");
  require(body(
    sources.journal,
    "ensure_same_request",
  ), "existing.provider_id != input.provider_id", "retained provider is immutable request identity");
  require(claim, "find_fulfillment_key_on_connection(", "claim fences all retained original-key collisions");
  before(
    execute,
    ".executable_provider(",
    ".claim_execution_with_stock_scope(",
    "unavailable registry is known-uninvoked before claim",
  );
  require(sources.admissionMigration, "OLD.status IN ('reconciliation_required', 'provider_error') AND OLD.committed_at IS NULL AND NEW.status = 'provider_succeeded'", "explicit known success promotion preserves retained SQL facts");
  const checkpoint = body(sources.journal, "mark_provider_succeeded");
  for (const marker of [
    "preserve_provider_reference(",
    "require_matching_provider_outcome(",
    ".mark_execution_reconciliation_required(",
    ".filter(reference_match)",
  ])
    require(checkpoint, marker, "provider outcomes remain monotonic across late success and duplicate callbacks");
  const quarantine = body(
    sources.journal,
    "mark_execution_reconciliation_required",
  );
  for (const marker of [
    "for _ in 0..2",
    "saved != reported",
    "Expr::col(provider_operation::Column::ProviderResult).into()",
    "Expr::col(provider_operation::Column::ProviderReference).into()",
    "Expr::col(provider_operation::Column::ProviderCompletedAt).into()",
    ".filter(result_match)",
    ".filter(reference_match)",
  ])
    require(quarantine, marker, "quarantine merges evidence without replacing recorded facts");
  for (const marker of [
    "OLD.provider_result IS NOT NULL AND NEW.provider_result IS DISTINCT FROM OLD.provider_result",
    "OLD.provider_result IS NOT NULL AND NEW.provider_result IS NOT OLD.provider_result",
  ])
    require(sources.admissionMigration, marker, "forward-only guard preserves every retained nonnull provider result");
  for (const source of [
    execute,
    body(sources.createAdmin, "execute_create_label"),
    body(sources.createBridge, "create_label"),
  ]) {
    const saved = spaced(source).slice(
      spaced(source).indexOf("let checkpoint ="),
    );
    require(saved, "checkpoint.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED", "automatic owner caller honors quarantined success checkpoint");
    require(saved, "explicitly reconcile local persistence", "automatic owner caller honors quarantined success checkpoint");
  }
  for (const source of [
    pin,
    body(sources.admin, "operation_request"),
    execute,
    recovery,
    body(sources.stock, "validate_provider_stock_context"),
  ])
    forbid(
      source,
      ".map(str::trim)",
      "retained literal identity is never trimmed into a different key",
    );
  forbid(
    recovery,
    "key.trim() != operation.idempotency_key",
    "retained literal identity is never trimmed into a different key",
  );
  forbid(
    body(sources.journal, "normalize_begin_input"),
    "input.idempotency_key = input.idempotency_key.trim()",
    "retained literal identity is never trimmed into a different key",
  );
  forbid(
    body(sources.stock, "validate_journal_local_input"),
    '.get("idempotency_key").and_then(Value::as_str).map(str::trim)',
    "retained literal identity is never trimmed into a different key",
  );
  const nullableStatus = body(
    sources.fulfillment,
    "reopened_status_for_cancelled",
  );
  require(nullableStatus, "fulfillment.shipped_at.is_some()", "nullable timestamp is not an ActiveValue receipt");
  forbid(
    nullableStatus,
    "fulfillment.shipped_at.clone().take().is_some()",
    "nullable timestamp is not an ActiveValue receipt",
  );
  const elapsedChecks =
    spaced(native).split(spaced("require_remaining_budget(context, started)?;"))
      .length - 1;
  if (elapsedChecks < 8)
    failures.push(
      "native elapsed budget before stock proof or writes: lost lock/read/write phase gates",
    );
  require(body(
    sources.inventory,
    "require_remaining_budget",
  ), ".checked_sub(started.elapsed())", "native elapsed budget before stock proof or writes");
  before(
    native,
    "require_remaining_budget(context, started)?",
    "scope::lock_lines(",
    "native elapsed budget before stock proof or writes",
  );
  const finish = body(sources.stock, "finish_progress_transition");
  before(
    finish,
    "complete(&txn, receipt, &response)",
    "remaining_stock_context(context, started)",
    "same budget survives receipt completion and final commit",
  );
  before(
    finish,
    "remaining_stock_context(context, started)",
    "txn.commit()",
    "same budget survives receipt completion and final commit",
  );
  const httpScope = body(sources.http, "admin_fulfillment_read_context");
  for (const marker of [
    "auth.tenant_id != tenant.id",
    "request_context.tenant_id != tenant.id",
    "tenant.id.is_nil()",
  ])
    require(httpScope, marker, "canonical request tenant matches authenticated owner scope");
  require(body(
    sources.graphql,
    "require_fulfillment_tenant_scope",
  ), ".is_some_and(|request| request.tenant_id != tenant.id)", "canonical request tenant matches authenticated owner scope");

  const evidence = JSON.parse(sources.evidence);
  if (
    evidence.contract?.provider_effect_admission_migration !== admissionName ||
    evidence.contract?.migration_execution !== "unexecuted"
  )
    failures.push(
      "evidence must name the unapplied forward-only provider admission rollout",
    );
  if (
    evidence.source_regressions?.rust_definition_count !== 58 ||
    evidence.source_regressions?.node_definition_count !== 30
  )
    failures.push(
      "focused regression source inventory must stay synchronized without claiming execution",
    );

  if (
    evidence.step8_complete !== false ||
    evidence.runtime_executed !== false ||
    evidence.source_regressions?.executed !== false ||
    evidence.source_regressions?.node_executed !== false
  )
    failures.push(
      "source evidence must not claim completion or regression/runtime execution",
    );
  for (const key of [
    "rust_compilation",
    "rust_regressions",
    "sql_postgresql_sqlite",
    "driver_contention_rollback",
    "external_carrier_outcomes",
    "mounted_transports",
    "remote_owner_convergence",
    "full_events_outbox_index",
    "unpaid_return_eligibility",
    "partial_inspection_topups",
    "source_guard_mutation_regressions",
    "migration_execution",
  ])
    if (evidence.unverified?.[key] !== true)
      failures.push(`unverified.${key} must remain true`);
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
  const result = verifyCommerceShipmentStockProvenance({
    root: process.env.RUSTOK_VERIFY_REPO_ROOT?.trim() || repository,
  });
  if (result.failures.length) {
    console.error(result.failures.join("\n"));
    process.exitCode = 1;
  } else
    console.log(
      "✔ Shipment provenance source contract; compilation, regressions, SQL/carriers/remote execution and full step 8 remain unverified",
    );
}
