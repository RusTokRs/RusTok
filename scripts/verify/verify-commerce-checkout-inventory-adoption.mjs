#!/usr/bin/env node

// Read-only source contract. No tests, SQL, Rust compilation, provider invocation or runtime
// certification occurs here. The companion mutation regressions remain maintainer-run.
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
  ports: "crates/modules/rustok-inventory/src/ports.rs",
  facade: "crates/modules/rustok-inventory/src/lib.rs",
  policy: "crates/libs/rustok-api/src/ports.rs",
  metadataBound:
    "crates/modules/rustok-inventory/src/migrations/m20260713_000021_bound_reservation_metadata_and_inactive_locations.rs",
  metadataCompaction:
    "crates/modules/rustok-inventory/src/migrations/m20260713_000022_repair_oversized_reservation_metadata.rs",
  admitted: "crates/modules/rustok-inventory/src/reservation_owner_context.rs",
  inventory: "crates/modules/rustok-inventory/src/reservation_adoption.rs",
  commerce:
    "crates/modules/rustok-commerce/src/services/checkout_inventory_order_adoption.rs",
  creation:
    "crates/modules/rustok-commerce/src/services/checkout_order_creation.rs",
  stages:
    "crates/modules/rustok-commerce/src/services/checkout_order_stages.rs",
  pipeline:
    "crates/modules/rustok-commerce/src/services/checkout_stage_pipeline_owner_ports.rs",
  staged: "crates/modules/rustok-commerce/src/services/staged_checkout.rs",
  recovery:
    "crates/modules/rustok-commerce/src/services/recovering_staged_checkout.rs",
  journal: "crates/modules/rustok-commerce/src/services/checkout_operation.rs",
  runtime:
    "crates/modules/rustok-commerce/src/storefront_staged_checkout_runtime.rs",
  registry:
    "crates/modules/rustok-inventory/contracts/inventory-fba-registry.json",
  matrix:
    "crates/modules/rustok-inventory/contracts/evidence/inventory-contract-test-static-matrix.json",
};
export const sourcePaths = Object.values(sourceFiles);

export function verifyCommerceCheckoutInventoryAdoption({
  root = repository,
} = {}) {
  const failures = [];
  const sources = Object.fromEntries(
    Object.entries(sourceFiles).map(([key, file]) => [
      key,
      fs
        .readFileSync(path.join(root, file), "utf8")
        .split(/#\[cfg\(test\)\]\s*mod tests\s*\{/)[0],
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
    "async fn adopt_inventory_by_identity(",
    "pub struct InventoryIdentityReservationAdoptionRequest",
    "pub struct InventoryIdentityReservationAdoptionSnapshot",
    "pub struct InventoryIdentityReservationAdoptionItem",
    "pub const MAX_RESERVATION_ADOPTION_ITEMS: usize = 1024",
    "pub checkout_operation_id: Uuid",
    "pub cart_id: Uuid",
    "pub order_id: Uuid",
    "pub reservation_id: Uuid",
    "pub external_id: String",
    "pub variant_id: Uuid",
    "pub quantity: i32",
    "pub location_id: Uuid",
    "pub cart_line_item_id: Uuid",
    "pub order_line_item_id: Uuid",
  ])
    require(sources.ports, marker, "one explicit owner command");
  const identityContract = sources.ports.slice(
    sources.ports.indexOf("pub trait InventoryReservationIdentityPort"),
    sources.ports.indexOf("pub struct InventoryAvailabilityRequest"),
  );
  require(identityContract, /async fn adopt_inventory_by_identity\(\s*&self,\s*context:\s*PortContext,\s*request:\s*InventoryIdentityReservationAdoptionRequest\s*,?\s*\)\s*->\s*Result<InventoryIdentityReservationAdoptionSnapshot,\s*PortError>;/, "adoption is mandatory on the existing identity capability, not a default or noop");
  for (const marker of [
    "context.require_policy(PortCallPolicy::write())?",
    "context.require_write_semantics()?",
    "crate::reservation_adoption::adopt_inventory_by_identity(&self.db, context, request).await",
  ])
    require(body(
      sources.ports.slice(
        sources.ports.indexOf(
          "impl InventoryReservationIdentityPort for PersistentInventoryReservationIdentityPort",
        ),
      ),
      "adopt_inventory_by_identity",
    ), marker, "persistent entry admission");
  for (const marker of [
    "mod reservation_adoption;",
    "InventoryIdentityReservationAdoptionRequest",
    "InventoryIdentityReservationAdoptionSnapshot",
  ])
    require(sources.facade, marker, "canonical private implementation and public DTOs");
  require(sources.facade, /#\[path\s*=\s*"ports.rs"\]\s*mod ports_impl;/, "persistent implementation has no public bypass");
  forbid(
    sources.facade,
    "pub mod ports_impl",
    "persistent implementation has no public bypass",
  );
  for (const marker of [
    "pub use crate::reservation_owner_context::{",
    "PersistentInventoryReservationIdentityPort, in_process_inventory_reservation_identity_port",
  ])
    require(sources.facade, marker, "both public entry paths share the admitted wrapper");
  require(body(
    sources.admitted,
    "new",
  ), "crate::ports_impl::PersistentInventoryReservationIdentityPort::new(db)", "only admitted wrapper constructs the persistent implementation");
  before(
    body(sources.admitted, "require_inventory_reservation_write_admission"),
    ".require_policy(PortCallPolicy::write())",
    "context.require_write_semantics()",
    "same ordered public admission",
  );
  require(body(
    sources.policy,
    "require_deadline_semantics",
  ), "self.deadline_ms.unwrap_or_default() == 0", "canonical positive deadline, not a zero timeout");
  const admitted = body(sources.admitted, "adopt_inventory_by_identity");
  for (const marker of [
    "require_inventory_reservation_write_admission(&context, ADOPT_OPERATION)?",
    "parse_inventory_reservation_tenant_id(&context, ADOPT_OPERATION)?",
    "InventoryReservationIdentityDiagnostic::adopt(&request)",
    ".adopt_inventory_by_identity(context, request)",
    "map_inventory_reservation_identity_local_port_error(",
  ])
    require(admitted, marker, "same admitted diagnostic boundary");
  const diagnostic = body(sources.admitted, "adopt");
  for (const marker of [
    "binding_count: request.items.len()",
    ".take(MAX_RESERVATION_ADOPTION_ITEMS + 1)",
    ".chars().take(192)",
  ])
    require(diagnostic, marker, "bounded batch request diagnostics");
  for (const marker of [
    "error = ?error",
    "request = ?request",
    "%request.external_id",
  ])
    forbid(
      admitted + diagnostic,
      marker,
      "no private delegated diagnostic payload",
    );

  const owner = body(sources.inventory, "adopt_inventory_by_identity");
  before(
    owner,
    "validate_request(&mut request)?",
    ".begin()",
    "refuse malformed commands before persistence",
  );
  before(
    owner,
    "load_tenant_variant(",
    "load_inventory_item_for_update(",
    "tenant proof before item locks",
  );
  before(
    owner,
    "load_inventory_item_for_update(",
    "reservation_item::Entity::find_by_id(",
    "same item lock as identity reserve/release",
  );
  for (const marker of [
    "collect::<BTreeSet<_>>()",
    "collect::<BTreeMap<_, _>>()",
    ".filter(reservation_item::Column::InventoryItemId.eq(item.id))",
    ".lock_exclusive()",
    "validate_saved_reservation(&request, binding, &reservation, item)?",
    ".filter(stock_location::Column::TenantId.eq(tenant_id))",
    "required.checked_add(reservation.quantity)",
    "inventory_level::Entity::find()",
    ".limit(2)",
    "let [level] = levels.as_slice()",
    "level.reserved_quantity < required",
    "if reservation.line_item_id != Some(binding.order_line_item_id)",
    "for (reservation, order_line_item_id, metadata) in changes",
    "active.line_item_id = Set(Some(order_line_item_id))",
    "active.metadata = Set(metadata)",
  ])
    require(owner, marker, "owner-local validation, allocation and replay invariant");
  before(
    owner,
    "for ((item_id, location_id), required) in required_by_level",
    "active.line_item_id =",
    "whole batch checked before binding writes",
  );
  before(
    owner,
    "txn.commit()",
    "Ok(InventoryIdentityReservationAdoptionSnapshot",
    "reply only after owner commit",
  );
  before(
    owner,
    "validate_adoption_metadata_capacity(&context, &txn, &changes).await?",
    "active.line_item_id =",
    "whole metadata batch fits before any binding writes",
  );
  const preparedMetadata = body(sources.inventory, "adoption_metadata");
  for (const marker of [
    "saved.as_object().cloned()",
    'metadata.insert("order_id"',
    'metadata.insert("order_line_item_id"',
  ])
    require(preparedMetadata, marker, "preserve full saved metadata and add only the target binding");
  const capacity = body(
    sources.inventory,
    "validate_adoption_metadata_capacity",
  );
  for (const marker of [
    "changes.is_empty()",
    "DbBackend::Postgres",
    "octet_length(value::text)",
    "jsonb_array_elements($1::jsonb)",
    "DbBackend::Sqlite",
    "length(CAST(value AS BLOB))",
    "json_each(?)",
    "serde_json::to_string(",
    "Statement::from_sql_and_values(",
    "vec![metadata.into()]",
    "validate_adoption_metadata_bytes(size.max_metadata_bytes)",
    '"inventory.adoption_backend_unsupported"',
  ])
    require(capacity, marker, "backend-native physical byte representation, not character counts or truncation");
  for (const source of [
    sources.inventory,
    sources.metadataBound,
    sources.metadataCompaction,
  ])
    require(source, /const MAX_RESERVATION_METADATA_BYTES:\s*(?:i64|usize)\s*=\s*32\s*\*\s*1024;/, "same physical metadata bound, without migration rewrites");
  for (const marker of [
    "bytes <= 0",
    "bytes > MAX_RESERVATION_METADATA_BYTES",
    '"inventory.adoption_metadata_too_large"',
    "PortError::conflict(",
  ])
    require(body(
      sources.inventory,
      "validate_adoption_metadata_bytes",
    ), marker, "deterministic metadata refusal is nonretryable");
  for (const marker of [
    "inventory_level::Entity::update",
    "inventory_level::ActiveModel",
    "stocked_quantity =",
    "reserved_quantity =",
    "active.quantity =",
    "active.location_id =",
    "active.external_id =",
    "InventoryService::new(",
    "reserve_inventory_by_identity(",
    "release_inventory_by_identity(",
    "Pricing",
    "is_metadata_visible_for_public_channel",
  ])
    forbid(
      sources.inventory,
      marker,
      "adoption is not stock allocation, refresh or movement",
    );
  const validation = body(sources.inventory, "validate_request");
  for (const marker of [
    "request.items.is_empty()",
    "request.items.len() > MAX_RESERVATION_ADOPTION_ITEMS",
    "validate_non_nil_uuid",
    "binding.quantity <= 0",
    "reservation_ids.insert",
    "external_ids.insert",
    "cart_line_ids.insert",
    "order_line_ids.insert",
    "cart_line_ids.is_disjoint(&order_line_ids)",
  ])
    require(validation, marker, "strict identity and batch bounds");
  const saved = body(sources.inventory, "validate_saved_reservation");
  for (const marker of [
    "reservation.id != binding.reservation_id",
    "item.variant_id != binding.variant_id",
    "reservation.external_id.as_deref() != Some(binding.external_id.as_str())",
    "reservation.location_id != binding.location_id",
    "reservation.deleted_at.is_some()",
    "reservation.quantity != binding.quantity",
    '"reservation_id"',
    '"external_id"',
    '"variant_id"',
    "checkout_provenance_matches(",
    '"cart_line_item_id"',
    '"order_id"',
    '"order_line_item_id"',
    'Some("inventory_identity_port" | "checkout_operation")',
  ])
    require(saved, marker, "unchanged saved allocation and known legacy provenance");
  require(saved, "has_terminal_disposition(&reservation.metadata)", "terminal metadata is not an active allocation or repair authority");
  for (const marker of [
    '"inventory_disposition"',
    '"superseded_by"',
    '"released_at"',
    '"disposed_at"',
    '"superseded_at"',
    "!value.is_null()",
  ])
    require(body(
      sources.inventory,
      "has_terminal_disposition",
    ), marker, "retained terminal metadata cannot be discarded or guessed active");
  const compacted = body(sources.inventory, "checkout_provenance_matches");
  for (const marker of [
    "None | Some(Value::Null)",
    'metadata.get("metadata_truncated")',
    "Some(true)",
    "metadata_uuid(metadata, key) == Some(expected)",
  ])
    require(compacted, marker, "only known compaction permits missing labels, never contradictions");

  for (const marker of [
    "inventory_item::Entity",
    "reservation_item::Entity",
    "reservation_item::ActiveModel",
    "merge_metadata(",
    "in_process_inventory_reservation_identity_port(",
  ])
    forbid(
      sources.commerce,
      marker,
      "Commerce must not resolve or mutate foreign Inventory rows",
    );
  for (const marker of [
    "inventory_port: Arc<dyn InventoryReservationIdentityPort>",
    "inventory_port,",
  ])
    require(body(sources.commerce, "new") +
      sources.commerce.slice(
        0,
        sources.commerce.indexOf("impl CheckoutInventoryOrderAdoptionService"),
      ), marker, "mandatory supplied provider");
  const stageConstructor = body(sources.stages, "new");
  require(stageConstructor, /CheckoutInventoryReservationExecutor::new\(\s*db\.clone\(\),\s*event_bus\.clone\(\),\s*inventory_port\.clone\(\),?\s*\)/, "selected provider reserves");
  require(stageConstructor, /CheckoutOrderCreationExecutor::new\(\s*db\.clone\(\),\s*event_bus\.clone\(\),\s*inventory_port,?\s*\)/, "same selected provider adopts");
  require(body(
    sources.creation,
    "new",
  ), /CheckoutInventoryOrderAdoptionService::new\(\s*db,\s*event_bus,\s*inventory_port,?\s*\)/, "provider threaded through order creation");
  const creation = body(sources.creation, "create_pending_and_adopt");
  for (const marker of [
    "saved_inventory_channel = channel_slug.clone()",
    "PortActor::user(actor_id.to_string())",
    "inventory_context.channel = saved_inventory_channel",
    ".with_deadline(self.port_deadline)",
    '.with_idempotency_key(format!("checkout:{operation_id}:inventory-adopt"))',
    ".adopt_and_checkpoint(inventory_context, operation_id, lease_owner, &order)",
  ])
    require(creation, marker, "frozen context and immutable mutation identity");
  const commerce = body(sources.commerce, "adopt_rows");
  for (const marker of [
    "mappings.len() > MAX_RESERVATION_ADOPTION_ITEMS",
    "mapping.location_id",
    ".filter(|id| !id.is_nil())",
    "items.sort_by_key(|item| item.cart_line_item_id)",
    "cart_id: operation.cart_id",
    "Column::ExternalId.eq(item.external_id.as_str())",
    "Column::Quantity.eq(item.quantity)",
    "Column::VariantId.eq(item.variant_id)",
    "Column::LocationId.eq(item.location_id)",
    "journal_result.map_err(after_owner_commit)",
  ])
    require(commerce, marker, "journal uses exactly the persisted allocation");
  before(
    commerce,
    "self.request_owner_adoption(context, &request).await?",
    "self.db.begin()",
    "distinct owner commit before Commerce journal transaction",
  );
  const invocation = body(sources.commerce, "request_owner_adoption");
  for (const marker of [
    ".min(30_000)",
    "owner_context.deadline_ms = Some(deadline_ms)",
    "tokio::time::timeout(",
    ".adopt_inventory_by_identity(owner_context, request.clone())",
    "validate_inventory_adoption_response(request, &response)",
  ])
    require(invocation, marker, "one bounded selected-owner invocation and full response correlation");
  before(
    invocation,
    ".require_deadline_semantics()",
    "tokio::time::timeout(",
    "missing or zero deadline refuses before the selected owner can be polled",
  );
  for (const marker of [
    "response.checkout_operation_id != request.checkout_operation_id",
    "response.cart_id != request.cart_id",
    "response.order_id != request.order_id",
    "response.items != request.items",
  ])
    require(body(
      sources.commerce,
      "validate_inventory_adoption_response",
    ), marker, "complete response, not inferred success");
  const boundary = body(sources.commerce, "inventory_adoption_boundary");
  require(boundary, "error.code.len() <= 100", "same error-code envelope as the operation journal");
  require(boundary, "retryable: error.retryable", "exact owner advice");
  for (const marker of ["error.message", "error = ?error"])
    forbid(
      boundary,
      marker,
      "provider message is not public or logged diagnostic data",
    );
  for (const marker of [
    "AfterOwnerCommit",
    "cause: Box<CheckoutInventoryOrderAdoptionError>",
  ])
    require(sources.commerce, marker, "explicit retained cause without a fake distributed rollback");
  require(sources.creation, "Self::Adoption(error) => error.bounded_failure()", "bounded error survives order creation");
  for (const [key, markers] of [
    [
      "pipeline",
      [
        "Self::OrderStage(stage)",
        "super::CheckoutOrderCreationError::Adoption(error)",
        "Some(error)",
      ],
    ],
    [
      "staged",
      [
        "Self::Pipeline(error) => error.inventory_adoption_failure()",
        "Self::PipelineAndJournal { pipeline, .. } => pipeline.inventory_adoption_failure()",
      ],
    ],
    [
      "recovery",
      [
        "Self::Staged(error) => error.inventory_adoption_failure()",
        "Self::StagedAndJournal { staged, .. }",
        "Self::StagedAndCompensation { staged, .. }",
        "staged.inventory_adoption_failure()",
      ],
    ],
  ])
    for (const marker of markers)
      require(body(
        sources[key],
        "inventory_adoption_failure",
      ), marker, "typed wrapper body retains the original adoption cause");
  require(body(
    sources.commerce,
    "bounded_failure",
  ), "Some(local_adoption_failure(self))", "a local pre-call failure cannot prove a prior attempt never committed");
  const recovery = body(sources.recovery, "complete_checkout");
  require(recovery, /\.is_some_and\(\s*super::CheckoutInventoryOrderAdoptionError::requires_owner_recovery\s*,?\s*\)/, "preserve typed same-identity failure before compensation lookup can erase it");
  before(
    recovery,
    "super::CheckoutInventoryOrderAdoptionError::requires_owner_recovery",
    "let operation = match self",
    "preserve typed same-identity failure before compensation lookup can erase it",
  );
  const failureMessage = body(sources.staged, "pipeline_failure_message");
  before(
    failureMessage,
    "error.inventory_adoption_failure()",
    "match error",
    "local source payload never becomes public journal text",
  );
  require(failureMessage, "Inventory adoption requires same-identity recovery", "static bounded adoption journal message");
  const disposition = body(sources.staged, "pipeline_failure_disposition");
  require(disposition, "error.inventory_adoption_failure()", "outcome uncertainty is separate from retry advice");
  require(disposition, "FailureDisposition::ReconciliationRequired", "false is not permission for compensation");
  require(sources.staged, ".mark_reconciliation_required(", "park through the owner journal");
  const parking = body(sources.journal, "mark_reconciliation_required");
  for (const marker of [
    "expected_status: CheckoutOperationStatus::Executing",
    "next_status: CheckoutOperationStatus::ReconciliationRequired",
    "error_code: error_code.into()",
    "self.release_lease_with_error(",
  ])
    require(parking, marker, "lease-fenced existing state and original code with atomic park events");
  const runtime = body(sources.runtime, "map_checkout_error");
  before(
    runtime,
    "error.inventory_adoption_failure()",
    "let (public, error_kind)",
    "unverified adoption never enters raw nested-error logging",
  );
  before(
    runtime,
    "return public",
    "let (public, error_kind)",
    "safe early public projection",
  );
  for (const marker of [
    "StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable",
    "StorefrontStagedCheckoutRuntimeError::ReconciliationRequired",
  ])
    require(runtime, marker, "same final advice for all existing storefront transports");

  const registry = JSON.parse(sources.registry),
    matrix = JSON.parse(sources.matrix);
  const port = registry.ports.find(
    (entry) => entry.name === "InventoryReservationIdentityPort",
  );
  if (!port?.operations.includes("adopt_inventory_by_identity"))
    failures.push(
      "identity adoption must be declared by the existing Inventory registry",
    );
  const declared = matrix.cases.find(
    (entry) => entry.operation === "adopt_inventory_by_identity",
  );
  if (
    matrix.status !== "static_matrix_locked" ||
    declared?.execution_status !== "static_locked_runtime_pending"
  )
    failures.push(
      "adoption source evidence must not become runtime/readiness evidence",
    );
  return {
    status: failures.length ? "failed" : "ok",
    failures,
    checked_files: sourcePaths,
  };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const result = verifyCommerceCheckoutInventoryAdoption();
  console.log(JSON.stringify(result, null, 2));
  if (result.failures.length) process.exitCode = 1;
}
