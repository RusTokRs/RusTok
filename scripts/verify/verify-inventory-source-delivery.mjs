#!/usr/bin/env node
// Read-only lexical/source check. No Rust/test callbacks/SQL/migrations/runtime are executed.
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
const inventory = "crates/modules/rustok-inventory";
export const files = {
  family: "crates/libs/rustok-events/src/inventory_source.rs",
  familyRegression: "crates/libs/rustok-events/src/inventory_source/tests.rs",
  eventRegistry: "crates/libs/rustok-events/src/lib.rs",
  wrapper: "crates/libs/rustok-events/src/contract.rs",
  publisher: `${inventory}/src/source_history/publication.rs`,
  writer: `${inventory}/src/source_history/writer.rs`,
  baseline: `${inventory}/src/source_history/baseline.rs`,
  reader: `${inventory}/src/source_history/reader.rs`,
  sourceApi: `${inventory}/src/source_history/mod.rs`,
  sourcePure: `${inventory}/src/source_history/tests.rs`,
  quantity: `${inventory}/src/services/quantity.rs`,
  reservation: `${inventory}/src/reservation_events.rs`,
  physical: `${inventory}/src/stock_events.rs`,
  physicalRegression: `${inventory}/src/source_history/physical_tests.rs`,
  bootstrap: `${inventory}/src/services/bootstrap.rs`,
  deletion: `${inventory}/src/services/bootstrap/deletion.rs`,
  adoption: `${inventory}/src/reservation_adoption.rs`,
  productCommands:
    "crates/modules/rustok-product/src/services/catalog/commands.rs",
  transactional: "crates/modules/rustok-outbox/src/transactional.rs",
  outboxTransport: "crates/modules/rustok-outbox/src/transport.rs",
  relay: "crates/modules/rustok-outbox/src/relay.rs",
  relayRegression:
    "crates/modules/rustok-outbox/tests/inventory_source_delivery.rs",
  moduleDelivery: "crates/libs/rustok-core/src/events/contract_delivery.rs",
  memoryTransport: "crates/libs/rustok-core/src/events/memory.rs",
  fanout: "apps/server/src/services/event_transport_factory.rs",
  generationTransport: "apps/server/src/services/tenant_cache_generation.rs",
  generationGate: "apps/server/src/services/tenant_generation_delivery_gate.rs",
  artifactTransport: "crates/modules/rustok-modules/src/event_delivery.rs",
  ownerRegression:
    "crates/modules/rustok-commerce/tests/inventory_source_history.rs",
  ownerTarget: "crates/modules/rustok-commerce/tests/catalog_service_test.rs",
  fbaRegistry: `${inventory}/contracts/inventory-fba-registry.json`,
  packet: `${inventory}/contracts/evidence/source-delivery-source.json`,
  companion: "scripts/verify/verify-inventory-source-delivery.test.mjs",
  package: "package.json",
  digestAdmission:
    "crates/libs/rustok-events/docs/event-contract-digest-admission.md",
};

// Ignore complete comment lines and inline test modules for production-body obligations.
// This is supplementary lexical evidence, not a Rust semantic parser or type checker.
function production(text) {
  return text.split(/#\[cfg\(test\)\]/)[0].replace(/^\s*\/\/.*$/gm, "");
}
export function verifyInventorySourceDelivery({ root = repository } = {}) {
  const failures = [];
  const read = (name) => {
    try {
      return fs.readFileSync(path.join(root, name), "utf8");
    } catch {
      failures.push(`missing ${name}`);
      return "";
    }
  };
  const source = Object.fromEntries(
    Object.entries(files).map(([key, name]) => [key, read(name)]),
  );
  const code = Object.fromEntries(
    Object.entries(source).map(([key, value]) => [key, production(value)]),
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
    if (!result) failures.push(`missing production function ${name}`);
    return result;
  };
  const before = (text, first, last, label) => {
    const normalized = spaced(text);
    const a = normalized.indexOf(spaced(first)),
      b = normalized.indexOf(spaced(last));
    if (a < 0 || b < 0 || a >= b) failures.push(`${label}: invalid order`);
  };

  for (const marker of [
    '"inventory.source.changed"',
    '"inventory.source.baseline_activated"',
    "deny_unknown_fields",
    "impl sealed::Sealed for InventorySourceEvent",
    "source_version: u64",
    "*source_version == 0 || *source_version > i64::MAX as u64",
    'validate_not_nil_uuid("variant_id", variant_id)',
    'validate_not_nil_uuid("baseline_id", baseline_id)',
    "ContractEventPayload::InventorySource(self)",
  ])
    require(code.family, marker, "sealed positive owner family");
  const declaration = code.family.slice(
    code.family.indexOf("pub enum InventorySourceEvent"),
    code.family.indexOf("impl InventorySourceEvent"),
  );
  for (const marker of [
    "stocked:",
    "quantity:",
    "product_id:",
    "actor_id:",
    "tenant_id:",
    "deleted:",
  ])
    forbid(
      declaration,
      marker,
      "source fact contains keys/version, not copied owner state/authority",
    );
  for (const marker of [
    "mod inventory_source;",
    "InventorySourceEvent",
    ".or_else(|| inventory_source_event_schema(event_type))",
    ".chain(INVENTORY_SOURCE_EVENT_SCHEMAS.iter())",
  ])
    require(code.eventRegistry, marker, "canonical export and both schema registries");
  for (const marker of [
    '#[serde(rename = "inventory_source")]',
    "InventorySource(InventorySourceEvent)",
    "Self::InventorySource(event) => event.event_type()",
    "Self::InventorySource(event) => event.schema_version()",
    "Self::InventorySource(event) => event.validate()",
  ])
    require(code.wrapper, marker, "exhaustive canonical typed dispatch");
  const policy = body(code.wrapper, "requires_module_delivery");
  require(policy, /matches!\(\s*self,\s*Self::InventoryReservation\(_\)\s*\|\s*Self::InventorySource\(_\)\s*\)/, "compiled required-module policy preserves unrelated remote routes");

  const publish = body(code.publisher, "publish");
  require(code.publisher, /txn:\s*&DatabaseTransaction/, "actual caller transaction");
  require(publish, /publish_contract_once_direct_in_tx_with_envelope_id\(\s*txn,\s*envelope_id,\s*tenant,\s*actor,\s*event,?\s*\)/, "canonical write-once uses retained identity and current actor");
  before(
    publish,
    "actor.is_some_and(|id| id.is_nil())",
    "publish_contract_once_direct_in_tx_with_envelope_id",
    "nil actor never silently becomes None",
  );
  for (const marker of [
    "ContractEventWriteOnceError::Conflict => E::StateUnverified",
    "ContractEventWriteOnceError::Unavailable => E::PublicationUnavailable",
    "saved != envelope_id",
  ])
    require(publish, marker, "bounded publication refusal");
  const changed = body(code.publisher, "changed");
  for (const marker of [
    "source_version <= 0",
    "u64::try_from(source_version)",
    "change_id,",
    "InventorySourceEvent::Changed",
  ])
    require(changed, marker, "exact positive change publication");
  require(body(
    code.publisher,
    "activated",
  ), /publish\(\s*txn,\s*tenant,\s*actor,\s*baseline_id,\s*InventorySourceEvent::BaselineActivated/, "activation uses original baseline identity");
  for (const marker of [
    ".commit(",
    ".begin(",
    "tokio::spawn",
    "ProductIndexRefreshEvent",
    "product_index_variant_refresh",
    "INSERT INTO index_",
    "UPDATE index_",
  ])
    forbid(
      [code.publisher, code.writer].join("\n"),
      marker,
      "no alternate publisher/transaction/Product authority/Index writes",
    );
  forbid(
    code.publisher,
    "Uuid::new",
    "publisher cannot replace retained identity",
  );
  const created = body(code.writer, "record_created");
  before(
    created,
    'insert(conn, tenant, baseline, snapshot, "created")',
    "super::publication::changed",
    "created journal before same-transaction source fact",
  );
  require(created, /publication::changed\(\s*conn,\s*tenant,\s*actor,\s*baseline,\s*variant,\s*1,\s*change_id\)/, "fresh identity's retained UUID and revision one");
  const update = body(code.writer, "update");
  for (const marker of [
    "next_version(saved.source_version)",
    "record::Column::SourceVersion.eq(saved.source_version)",
    "record::Column::LastChangeId.eq(saved.last_change_id)",
    "result.rows_affected != 1",
  ])
    require(update, marker, "exact source revision/head CAS");
  before(
    update,
    "append(",
    "super::publication::changed",
    "retained journal before canonical fact",
  );
  require(update, /publication::changed\(\s*conn,\s*saved\.tenant_id,\s*actor,\s*saved\.baseline_id,\s*saved\.variant_id,\s*version,\s*id,?\s*\)/, "same committed source version/UUID, never Product revision");
  require(code.writer, "update(self.conn, saved, 0, 0, 0, true, self.actor)", "deletion consumes original borrowed transaction and current actor");
  for (const text of [code.reader, code.adoption]) {
    forbid(text, "publication::", "reads/adoption are not publishers");
    forbid(
      text,
      "source_history::record_",
      "reads/adoption do not mint versions",
    );
  }
  const advance = body(code.baseline, "advance_in");
  before(
    advance,
    "if let Some(receipt)",
    "super::publication::activated",
    "original page replay precedes any publication",
  );
  before(
    advance,
    "update.exec(txn).await?.rows_affected != 1",
    "super::publication::activated",
    "activation fact follows exact checkpoint CAS",
  );
  require(advance, /if saved\.active\s*\{\s*super::publication::activated\(txn, tenant, publication_actor, saved\.baseline_id\)\.await\?;/, "only fresh final page activation publishes");
  forbid(
    body(code.baseline, "start_in"),
    "publication::",
    "status/start never reconstruct earlier delivery",
  );
  require(body(
    code.baseline,
    "advance_baseline",
  ), "crate::stock_events::event_actor_id(context)", "activation actor from current context, not saved creator");
  forbid(
    body(code.writer, "insert"),
    "publication::",
    "staged baseline rows are not ready notifications",
  );

  for (const [key, marker] of [
    [
      "quantity",
      /record_changed\(\s*txn,\s*self\.tenant_id,\s*self\.variant_id,\s*Some\(self\.actor_id\),?\s*\)/,
    ],
    [
      "reservation",
      /pub\(crate\) async fn publish\(self, txn: &DatabaseTransaction\)/,
    ],
    [
      "reservation",
      "record_changed(txn, self.tenant_id, *variant_id, self.actor_id)",
    ],
    [
      "physical",
      "record_changed(txn, self.tenant_id, variant_id, self.actor_id)",
    ],
    [
      "bootstrap",
      /record_created\(\s*conn,\s*location\.tenant_id,\s*input\.variant_id,\s*Some\(actor_id\),?\s*\)/,
    ],
    [
      "deletion",
      /prepare_deletion\(\s*txn,\s*tenant_id,\s*variant_id,\s*item_id,\s*Some\(actor_id\),?\s*\)/,
    ],
  ])
    require(code[key], marker, `actual ${key} current-actor writer`);
  before(
    body(code.bootstrap, "create_initial_records_in_tx"),
    "actor_id.is_nil()",
    "inventory_item::ActiveModel",
    "bootstrap actor validation before writes",
  );
  before(
    body(code.deletion, "delete_records"),
    "actor_id.is_nil()",
    "DeletionPlan::load",
    "delete actor validation before plan/effects",
  );
  for (const name of ["create_product", "create_variant"])
    require(body(
      code.productCommands,
      name,
    ), /BootstrapService::create_initial_records_in_tx\(\s*&txn,\s*&default_stock_location,\s*actor_id,/, `${name} forwards current actor`);
  for (const name of ["delete_product", "delete_variant"])
    require(body(
      code.productCommands,
      name,
    ), /BootstrapService::delete_records_for_variants_in_tx\(\s*&txn,\s*tenant_id,\s*actor_id,/, `${name} forwards current actor`);

  const fanout = body(code.fanout, "publish_contract");
  before(
    fanout,
    "if payload.requires_module_delivery() && !deliver_locally",
    "self.primary.publish_contract",
    "required unmatched family refuses before broker-only success",
  );
  for (const marker of [
    "self.local.event_bus().has_contract_handler(&envelope)?",
    "return Err(error);",
    "self.local.publish_contract(envelope).await",
  ])
    require(fanout, marker, "actual acknowledged fan-out");
  for (const marker of [
    "handlers.is_empty()",
    "handler.handle_contract(envelope).await",
    "result?;",
    "try_acquire_owned",
    "self.require_active()?",
  ])
    require(code.moduleDelivery, marker, "bounded awaited module handling");
  require(code.memoryTransport, "self.bus.publish_contract_envelope(envelope).await", "memory profile uses real module delivery");
  require(body(
    code.moduleDelivery,
    "publish_contract_envelope",
  ), "state.deliver(&envelope).await?", "memory dispatch propagates module completion");
  for (const key of ["generationTransport", "generationGate"])
    require(body(
      code[key],
      "publish_contract",
    ), "self.inner.publish_contract(envelope).await", "generation wrapper propagates sealed result");
  require(body(
    code.artifactTransport,
    "publish_contract",
  ), "self.downstream.publish_contract(envelope).await", "artifact intake cannot hide downstream failure");
  require(code.fanout, "transport_with_local_delivery(transport, channel_capacity)", "configured Iggy profile selects fenced fan-out");
  for (const marker of [
    "RelayEnvelope::Contract(envelope) => self.target.publish_contract(envelope).await",
    "ContractEventEnvelope",
    "validate_registered_schema",
    "self.mark_dispatched(model).await?",
  ])
    require(code.relay, marker, "real canonical relay decoding and completion");
  require(code.transactional, "OutboxTransport::write_contract_envelope_once_in_tx(txn, envelope).await", "one canonical outbox writer");
  require(code.outboxTransport, "write_contract_envelope_once_in_tx", "durable write-once admission");
  require(source.fbaRegistry, '"source_delivery_source"', "FBA source-only evidence discovery");
  require(source.fbaRegistry, '"source_delivery_guard"', "FBA read-only source check discovery");
  require(source.ownerTarget, "mod inventory_source_history;", "registered real owner regression target");
  require(source.package, '"verify:inventory:source-delivery"', "read-only discovery");
  require(source.digestAdmission, "Pending Inventory source admission (2026-11-02)", "canonical digest admission explicitly pending");

  let packet = {};
  try {
    packet = JSON.parse(source.packet);
  } catch {
    failures.push("invalid source packet");
  }
  if (
    packet.packet !== "inventory-source-delivery-source" ||
    packet.status !== "source_only_uncompiled_unrun"
  )
    failures.push("packet identity/status drift");
  for (const key of [
    "compiled",
    "tests_executed",
    "sql_executed",
    "migrations_executed",
    "digest_generated",
    "runtime_verified",
    "installation_baseline_initialized",
    "distribution_selected",
    "source_context_authority_bound",
    "initial_replay_durable",
    "index_persist_before_ack_verified",
    "index_ready",
    "step8_complete",
  ])
    if (packet[key] !== false) failures.push(`must not promote ${key}`);
  for (const key of [
    "canonical_versioned_publication_wired",
    "canonical_relay_dispatch_wired",
    "required_module_delivery_fenced",
  ])
    if (packet.source_wiring?.[key] !== true)
      failures.push(`source wiring drift: ${key}`);
  let definitions = 0;
  for (const [name, names] of Object.entries(packet.regressions?.rust ?? {}))
    for (const nameOfTest of names) {
      require(read(name), `fn ${nameOfTest}(`, "focused unrun definition");
      definitions++;
    }
  if (
    definitions !== 26 ||
    packet.regressions?.rust_definitions !== 26 ||
    packet.regressions?.node_definitions !== 10 ||
    packet.regressions?.executed !== false ||
    (source.companion.match(/^test\(/gm) ?? []).length !== 10
  )
    failures.push("unrun regression inventory drift");
  for (const [name, expected] of Object.entries(packet.preserved_hashes ?? {}))
    if (
      crypto.createHash("sha256").update(read(name)).digest("hex") !== expected
    )
      failures.push(`immutable historical payload changed: ${name}`);
  if (Object.keys(packet.preserved_hashes ?? {}).length !== 21)
    failures.push("historical payload/migration/digest pins missing");
  return {
    failures,
    rust_definitions: definitions,
    node_definitions: 10,
    testsExecuted: false,
    runtimeVerified: false,
  };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const result = verifyInventorySourceDelivery();
  if (result.failures.length) {
    result.failures.forEach((failure) => console.error(failure));
    process.exitCode = 1;
  } else
    console.log(
      "Inventory source canonical producer/relay source guard passed; 26 Rust/10 Node definitions unrun; selected Distribution/Index, digest/compiler/backend/runtime remain pending.",
    );
}
