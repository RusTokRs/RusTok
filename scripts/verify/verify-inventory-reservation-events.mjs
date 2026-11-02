#!/usr/bin/env node

// Read-only source contract. Never imports regression companions, executes Rust/SQL, or
// generates schema digests. Transport doubles/markers are not delivery/readiness evidence.
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
  family: "crates/libs/rustok-events/src/inventory_reservation.rs",
  wrapper: "crates/libs/rustok-events/src/contract.rs",
  registry: "crates/libs/rustok-events/src/lib.rs",
  root: "crates/libs/rustok-events/src/types/domain_event.rs",
  publication: "crates/modules/rustok-inventory/src/reservation_events.rs",
  service: "crates/modules/rustok-inventory/src/services/inventory.rs",
  ports: "crates/modules/rustok-inventory/src/ports.rs",
  actor: "crates/modules/rustok-inventory/src/stock_events.rs",
  adoption: "crates/modules/rustok-inventory/src/reservation_adoption.rs",
  native:
    "crates/modules/rustok-inventory/admin/src/transport/native_server_adapter.rs",
  transactional: "crates/modules/rustok-outbox/src/transactional.rs",
  core: "crates/libs/rustok-core/src/events/contract_delivery.rs",
  bus: "crates/libs/rustok-core/src/events/bus.rs",
  handler: "crates/libs/rustok-core/src/events/handler.rs",
  memory: "crates/libs/rustok-core/src/events/memory.rs",
  factory: "apps/server/src/services/event_transport_factory.rs",
  dispatcher: "apps/server/src/services/module_event_dispatcher.rs",
  generation: "apps/server/src/services/tenant_cache_generation.rs",
  gate: "apps/server/src/services/tenant_generation_delivery_gate.rs",
  artifacts: "crates/modules/rustok-modules/src/event_delivery.rs",
  ingestion: "crates/modules/rustok-search/src/ingestion.rs",
  projector: "crates/modules/rustok-search/src/projector_core.rs",
  persistence:
    "crates/modules/rustok-commerce/tests/inventory_reservation_events.rs",
  target: "crates/modules/rustok-commerce/tests/inventory_service_test.rs",
  evidence:
    "crates/modules/rustok-inventory/contracts/evidence/reservation-events-source.json",
  companion: "scripts/verify/verify-inventory-reservation-events.test.mjs",
};
export const sourcePaths = Object.values(sourceFiles);

export function verifyInventoryReservationEvents({ root = repository } = {}) {
  const failures = [];
  const sources = Object.fromEntries(
    Object.entries(sourceFiles).map(([name, file]) => [
      name,
      fs.readFileSync(path.join(root, file), "utf8"),
    ]),
  );
  const runtime = (source) =>
    source.split(
      /#\[cfg\(test\)\]\s*(?:#\[path\s*=\s*"[^"\n]+"\]\s*)?mod\s+\w+\s*[;{]/,
    )[0];
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
  const family = runtime(sources.family),
    publication = runtime(sources.publication);
  for (const marker of [
    "inventory.reservation.changed",
    "INVENTORY_RESERVATION_EVENT_SCHEMA_VERSION: u16 = 1",
    "impl sealed::Sealed",
    "impl EventContract",
    "impl ValidateEvent",
    'validate_not_nil_uuid("product_id"',
    'validate_not_nil_uuid("variant_id"',
    "deny_unknown_fields",
  ])
    require(family, marker, "sealed quantity-free contract");
  for (const marker of [
    "old_quantity:",
    "new_quantity:",
    "quantity: i32",
    "source_version:",
    "metadata:",
  ])
    forbid(
      family,
      marker,
      "no snapshot, receipt or Product revision impersonation",
    );
  require(sources.wrapper, "InventoryReservation(InventoryReservationEvent)", "canonical typed wrapper");
  forbid(sources.root, "InventoryReservationEvent", "root family unchanged");
  require(sources.registry, ".or_else(|| inventory_reservation_event_schema(event_type))", "registered lookup");
  require(sources.registry, ".chain(INVENTORY_RESERVATION_EVENT_SCHEMAS.iter())", "registered digest input");
  for (const marker of [
    "quantity_tenant(context, operation)",
    "event_actor_id(context)",
    "variant.tenant_id != tenant_id",
    "product::Column::TenantId.eq(tenant_id)",
    "InventoryReservationEvent::Changed",
    "TransactionalEventBus::publish_contract_direct_in_tx(",
  ])
    require(publication, marker, "current-context owner publication");
  for (const marker of [
    "DomainEvent::InventoryUpdated",
    "ProductIndexRefreshEvent",
    "StockEventPlan",
    "serde_json",
    "sys_events",
    "saturating_",
  ])
    forbid(
      publication,
      marker,
      "single canonical quantity-free owner publisher",
    );
  require(sources.actor, "PortActorKind::Service | PortActorKind::System => Ok(None)", "no fabricated service users");
  forbid(
    runtime(sources.adoption),
    "ReservationEventPlan",
    "adoption is binding-only",
  );

  const service = runtime(sources.service);
  for (const name of ["reserve", "release_reservation_quantity"]) {
    require(service, new RegExp(
      `pub async fn ${name}\\(\\s*&self,\\s*context: &PortContext,`,
    ), "native context required");
    const method = body(service, name);
    before(
      method,
      "if quantity == 0",
      "ReservationEventPlan::prepare",
      "zero has no new publication dependency",
    );
    const movement = method.slice(method.indexOf("let event_plan"));
    before(
      movement,
      "event_plan.publish(&txn)",
      "txn.commit()",
      "fact in the same owner transaction",
    );
  }
  const quantity = sources.ports.slice(
    sources.ports.indexOf("impl InventoryReservationPort for"),
    sources.ports.indexOf("impl InventoryReservationIdentityPort for"),
  );
  require(body(
    quantity,
    "reserve_inventory",
  ), ".reserve(&context, request.variant_id, request.quantity)", "quantity actor/context forwarding");
  require(body(
    quantity,
    "release_inventory_reservation",
  ), ".release_reservation_quantity(&context, request.variant_id, request.quantity)", "quantity actor/context forwarding");
  const identity = sources.ports.slice(
    sources.ports.indexOf("impl InventoryReservationIdentityPort for"),
  );
  for (const name of [
    "reserve_inventory_by_identity",
    "release_inventory_by_identity",
  ]) {
    const method = body(identity, name);
    before(
      method,
      name === "reserve_inventory_by_identity"
        ? "existing_reservation_snapshot("
        : "if reservation.deleted_at.is_some()",
      "ReservationEventPlan::prepare",
      "identity replay precedes new dependencies",
    );
    const movement = method.slice(method.indexOf("let event_plan"));
    before(
      movement,
      "event_plan.publish(&txn)",
      "txn.commit()",
      "identity movement and fact are atomic",
    );
  }
  for (const marker of [
    "checked_add(request.quantity)",
    ".exec_without_returning(&txn)",
    "*affected != 1",
    "updated.rows_affected != 1",
  ])
    require(identity, marker, "verified fresh effects before publication");
  require(sources.native, "rustok_api::PortActor::user(actor_id.to_string())", "current authenticated native actor");
  require(sources.native, "reservation_write_context(", "native context mapping");
  require(body(
    sources.ports,
    "available_quantity",
  ), "crate::services::inventory::checked_available_quantity(&levels)", "one signed checked owner evaluator");
  forbid(
    runtime(sources.ports),
    ".saturating_sub(",
    "identity results cannot clamp negative availability",
  );
  const direct = body(sources.transactional, "publish_contract_direct_in_tx");
  require(direct, "build_contract_envelope(tenant_id, actor_id, None, event)", "canonical sealed construction");
  require(direct, "OutboxTransport::write_contract_envelope_in_tx(txn, envelope)", "existing canonical outbox persistence");

  const core = runtime(sources.core);
  for (const marker of [
    "Weak::upgrade",
    "Arc::downgrade(&state)",
    "try_acquire_owned()",
    "max_concurrent > Semaphore::MAX_PERMITS",
    "handlers.is_empty()",
    "handler.handle_contract(envelope).await",
    "result?",
    "impl Drop for ContractListenerRegistration",
  ])
    require(core, marker, "acknowledged bounded local delivery");
  for (const marker of [
    "InventoryReservation",
    "broadcast::channel",
    "mpsc::",
    "serde_json",
    "tokio::spawn",
  ])
    forbid(
      core,
      marker,
      "domain-neutral delivery, not a second queue/pipeline",
    );
  before(
    body(core, "deliver"),
    "handler.handle_contract(envelope).await",
    "result?",
    "handler failure is not acknowledgement",
  );
  require(sources.memory, "self.bus.publish_contract_envelope(envelope).await", "actual Memory transport delivery");
  require(sources.handler, "start_with_contract_delivery", "explicit registered consumer opt-in");
  require(sources.handler, "self.config.max_queue_depth > Semaphore::MAX_PERMITS", "invalid queue bounds refuse before binding");
  require(sources.handler, "let _contract_registration = registration", "binding follows task abort/unwind");
  require(sources.dispatcher, "dispatcher.start_with_contract_delivery()?", "production module listener startup");
  const fanout = body(
    sources.factory.slice(
      sources.factory.indexOf(
        "impl EventTransport for LocalDeliveryFanoutTransport",
      ),
    ),
    "publish_contract",
  );
  before(
    fanout,
    "has_contract_handler(&envelope)?",
    "self.primary.publish_contract",
    "do not skip local targets before startup",
  );
  before(
    fanout,
    "self.primary.publish_contract",
    "self.local.publish_contract",
    "Iggy acceptance before local delivery",
  );
  require(fanout, "return Err(error)", "retain durable outbox on local failure");
  require(fanout, "self.primary.publish_contract(envelope.clone()).await?", "preserve the original remote envelope identity");
  for (const key of ["generation", "gate", "artifacts"])
    require(sources[
      key
    ], ".publish_contract(envelope).await", "real wrapper typed forwarding");
  require(body(
    sources.artifacts,
    "project_contract",
  ), "self.project_source(", "artifact subscriptions are not silently skipped");
  require(body(
    sources.artifacts,
    "project_contract",
  ), "event_id: envelope.id()", "artifact dedupe retains the original identity");

  const ingestion = sources.ingestion.slice(
    sources.ingestion.indexOf("impl EventHandler for SearchIngestionHandler"),
  );
  require(body(
    ingestion,
    "handles_contract",
  ), "ContractEventPayload::InventoryReservation", "explicit Search consumer");
  require(body(
    ingestion,
    "handle_contract",
  ), ".upsert_product(envelope.tenant_id(), *product_id)", "authoritative Search refetch");
  const lock = body(sources.projector, "begin_product_transaction");
  for (const marker of [
    "IsolationLevel::ReadCommitted",
    "pg_advisory_xact_lock",
    "rustok-search:products:{tenant_id}",
  ])
    require(lock, marker, "serialize refresh before fresh source reads");
  for (const name of [
    "upsert_product",
    "rebuild_product_scope",
    "delete_product",
  ])
    require(body(
      sources.projector,
      name,
    ), "begin_product_transaction(tenant_id)", "all Search product-scope writes coordinate");
  require(sources.projector, "SUM(il.stocked_quantity::bigint - il.reserved_quantity::bigint)", "signed refetch arithmetic");
  require(sources.target, '#[path = "inventory_reservation_events.rs"]', "registered regression child");
  if ((sources.persistence.match(/#\[tokio::test\]/g) ?? []).length !== 15)
    failures.push(
      "15 native persistence definitions must remain registered, not claimed executed",
    );
  if ((sources.companion.match(/\btest\(/g) ?? []).length !== 10)
    failures.push(
      "10 Node regression definitions must remain separate from source guard execution",
    );
  const evidence = JSON.parse(sources.evidence);
  if (
    evidence.compiled !== false ||
    evidence.sql_executed !== false ||
    evidence.runtime_certified !== false ||
    evidence.step8_complete !== false ||
    evidence.generic_index_complete !== false
  )
    failures.push(
      "source evidence must not claim compilation, runtime, Index or step-8 completion",
    );
  if (
    evidence.source_regressions?.rust_definition_count !== 34 ||
    evidence.source_regressions?.rust_executed !== 0 ||
    evidence.source_regressions?.node_definition_count !== 10 ||
    evidence.source_regressions?.node_executed !== 0
  )
    failures.push(
      "34 Rust/10 Node source-only regression counts must remain honest",
    );
  if (
    evidence.schema_digest?.regenerated !== false ||
    evidence.schema_digest?.artifact_modified !== false
  )
    failures.push(
      "this source handoff has no Rust-generated digest evidence; do not hand-author the artifact",
    );
  return {
    failures,
    source_only: true,
    compiled: false,
    tests_executed: false,
    sql_executed: false,
    step8_complete: false,
  };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const result = verifyInventoryReservationEvents();
  if (result.failures.length) {
    for (const failure of result.failures) console.error(`✖ ${failure}`);
    process.exitCode = 1;
  } else {
    console.log(
      "✔ Inventory reservation fact/delivery source contract; 34 Rust/10 Node definitions unrun, digests/compilation/runtime/generic Index/step 8 remain open",
    );
  }
}
