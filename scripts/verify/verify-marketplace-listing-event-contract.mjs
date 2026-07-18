import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  contract: "crates/rustok-events/src/contract.rs",
  listing: "crates/rustok-events/src/marketplace_listing.rs",
  exports: "crates/rustok-events/src/lib.rs",
  coreTransport: "crates/rustok-core/src/events/transport.rs",
  outboxBus: "crates/rustok-outbox/src/transactional.rs",
  outboxTransport: "crates/rustok-outbox/src/transport.rs",
  outboxRelay: "crates/rustok-outbox/src/relay.rs",
  iggySerializer: "crates/rustok-iggy/src/serialization.rs",
  iggyProducer: "crates/rustok-iggy/src/producer.rs",
  iggyTransport: "crates/rustok-iggy/src/transport.rs",
  iggyContractConsumer: "crates/rustok-iggy/src/contract_consumer.rs",
  iggyExports: "crates/rustok-iggy/src/lib.rs",
  tests: "crates/rustok-events/tests/marketplace_listing_contracts.rs",
  registry: "crates/rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json",
  ownerService: "crates/rustok-marketplace-listing/src/service.rs",
  ownerReceipts: "crates/rustok-marketplace-listing/src/command_receipts.rs",
  ownerEvents: "crates/rustok-marketplace-listing/src/external_events.rs",
  ownerEvented: "crates/rustok-marketplace-listing/src/evented_commands.rs",
  ownerLifecycle: "crates/rustok-marketplace-listing/src/lifecycle_event_commands.rs",
  ownerReplay: "crates/rustok-marketplace-listing/src/replay_safe_commands.rs",
  ownerTests: "crates/rustok-marketplace-listing/src/command_receipts_tests.rs",
};

const events = [
  ["MarketplaceListingCreated", "marketplace.listing.created"],
  ["MarketplaceListingTermsUpdated", "marketplace.listing.terms_updated"],
  ["MarketplaceListingSubmittedForReview", "marketplace.listing.submitted_for_review"],
  ["MarketplaceListingApproved", "marketplace.listing.approved"],
  ["MarketplaceListingRejected", "marketplace.listing.rejected"],
  ["MarketplaceListingPublished", "marketplace.listing.published"],
  ["MarketplaceListingSuspended", "marketplace.listing.suspended"],
  ["MarketplaceListingReactivated", "marketplace.listing.reactivated"],
  ["MarketplaceListingArchived", "marketplace.listing.archived"],
];

const failures = [];
const read = (file) => {
  const absolute = path.join(root, file);
  if (!fs.existsSync(absolute)) {
    failures.push(`${file}: missing`);
    return "";
  }
  return fs.readFileSync(absolute, "utf8");
};
const requireMarker = (source, marker, file) => {
  if (!source.includes(marker)) failures.push(`${file}: missing ${marker}`);
};
const forbidMarker = (source, marker, file) => {
  if (source.includes(marker)) failures.push(`${file}: forbidden ${marker}`);
};

const sources = Object.fromEntries(
  Object.entries(files).map(([key, file]) => [key, read(file)]),
);

for (const marker of [
  "pub(crate) mod sealed",
  "pub trait EventContract:",
  "sealed::Sealed",
  "pub enum ContractEventPayload",
  "MarketplaceListing(MarketplaceListingEvent)",
  "pub struct ContractEventEnvelope",
  "event.validate()?",
  "event.into_contract_payload()",
  "pub fn payload(&self) -> Result<&ContractEventPayload",
  "pub fn into_payload(self) -> Result<ContractEventPayload",
  "self.event.validate()?",
  "validate_registered_schema",
  "into_root_envelope",
]) requireMarker(sources.contract, marker, files.contract);
for (const forbidden of ["event: Value", "serde_json::Value"]) {
  forbidMarker(sources.contract, forbidden, files.contract);
}

requireMarker(sources.exports, "pub use contract::", files.exports);
requireMarker(sources.exports, "ContractEventPayload", files.exports);
requireMarker(sources.exports, "MarketplaceListingEvent", files.exports);
requireMarker(sources.exports, "pub fn event_schema", files.exports);
requireMarker(sources.exports, "pub fn event_schemas", files.exports);

for (const [variant, eventType] of events) {
  requireMarker(sources.listing, `${variant} => "${eventType}"`, files.listing);
  requireMarker(sources.listing, "Self::$variant", files.listing);
  requireMarker(sources.registry, `"${eventType}"`, files.registry);
  requireMarker(sources.ownerEvents, variant, files.ownerEvents);
}

for (const marker of [
  "impl sealed::Sealed for MarketplaceListingEvent",
  "impl EventContract for MarketplaceListingEvent",
  "ContractEventPayload::MarketplaceListing(self)",
  "impl ValidateEvent for MarketplaceListingEvent",
  'validate_scope_slug("market_slug", market_slug)?',
  'validate_scope_slug("channel_slug", channel_slug)?',
  'validators::validate_not_nil_uuid("listing_id", listing_id)?',
  'validators::validate_not_nil_uuid("seller_id", seller_id)?',
  'validators::validate_not_nil_uuid("master_product_id", master_product_id)?',
  'validators::validate_not_nil_uuid("master_variant_id", master_variant_id)?',
  'validators::validate_range("terms_version"',
  "MARKETPLACE_LISTING_EVENT_SCHEMAS",
]) requireMarker(sources.listing, marker, files.listing);

for (const forbidden of [
  "note:",
  "reason:",
  "metadata:",
  "approval_note:",
  "suspension_reason:",
]) forbidMarker(sources.listing, forbidden, files.listing);

for (const marker of [
  "async fn publish_contract",
  "ContractEventEnvelope",
  "into_root_envelope",
]) requireMarker(sources.coreTransport, marker, files.coreTransport);

for (const marker of [
  "pub async fn publish_contract_in_tx",
  "E: EventContract",
  "ContractEventEnvelope::new",
  "write_contract_to_outbox",
]) requireMarker(sources.outboxBus, marker, files.outboxBus);
requireMarker(
  sources.outboxTransport,
  "pub async fn write_contract_to_outbox",
  files.outboxTransport,
);
requireMarker(
  sources.outboxTransport,
  "model_from_contract_envelope",
  files.outboxTransport,
);
for (const marker of [
  "enum RelayEnvelope",
  "from_value::<ContractEventEnvelope>",
  "from_value::<EventEnvelope>",
  "self.target.publish_contract(envelope)",
  "sealed_contract_envelope_is_dispatched_without_root_deserialization",
]) requireMarker(sources.outboxRelay, marker, files.outboxRelay);

for (const marker of [
  "serialize_contract",
  "deserialize_contract",
  "contract_envelope_roundtrips_in_both_formats",
]) requireMarker(sources.iggySerializer, marker, files.iggySerializer);
for (const marker of [
  "build_contract_publish_request",
  "serializer.serialize_contract",
  "contract_event_routes_to_domain_without_root_event_deserialization",
]) requireMarker(sources.iggyProducer, marker, files.iggyProducer);
for (const marker of [
  "async fn publish_contract",
  "producer::build_contract_publish_request",
  "open_persistent_contract_consumer_group",
  "PersistentContractConsumerGroup::new",
]) requireMarker(sources.iggyTransport, marker, files.iggyTransport);
for (const marker of [
  "pub struct ConsumedContractEvent",
  "pub struct PersistentContractConsumerGroup",
  "deserialize_contract",
  "validate_registered_schema",
  "pub async fn acknowledge",
]) requireMarker(
  sources.iggyContractConsumer,
  marker,
  files.iggyContractConsumer,
);
for (const marker of [
  "pub mod contract_consumer;",
  "ConsumedContractEvent",
  "PersistentContractConsumerGroup",
]) requireMarker(sources.iggyExports, marker, files.iggyExports);

for (const marker of [
  "event_bus: TransactionalEventBus",
  "pub(crate) fn event_bus(&self) -> &TransactionalEventBus",
]) requireMarker(sources.ownerService, marker, files.ownerService);
for (const [source, file] of [
  [sources.ownerEvented, files.ownerEvented],
  [sources.ownerLifecycle, files.ownerLifecycle],
  [sources.ownerReplay, files.ownerReplay],
]) requireMarker(source, "self.event_bus().clone()", file);
forbidMarker(sources.ownerReceipts, "OutboxTransport::new", files.ownerReceipts);

for (const marker of [
  "event_for_completed_command(command_kind.as_str(), response)",
  "publish_contract_in_tx(&transaction, tenant_id, Some(actor_id), event)",
  "transaction.rollback().await?",
  "transaction.commit().await?",
]) requireMarker(sources.ownerReceipts, marker, files.ownerReceipts);
for (const marker of [
  "completed_receipt_commits_one_contract_event_and_replay_adds_none",
  "missing_outbox_storage_rolls_back_the_pending_receipt",
  "receipt_completion_failure_rolls_back_the_inserted_outbox_event",
]) requireMarker(sources.ownerTests, marker, files.ownerTests);
for (const forbidden of [
  '"legacy_snapshot" =>',
  "LegacyApprovalSnapshot",
  "LegacySuspensionSnapshot",
]) forbidMarker(sources.ownerEvents, forbidden, files.ownerEvents);

forbidMarker(sources.outboxBus, "event_type: String", files.outboxBus);
forbidMarker(sources.outboxBus, "payload: serde_json::Value", files.outboxBus);

for (const marker of [
  "listing_event_family_has_nine_registered_versioned_contracts",
  "listing_event_rejects_noncanonical_scope_and_invalid_version",
  "decoded_listing_envelope_revalidates_payload_fields",
  "listing_external_payload_excludes_owner_private_prose_and_metadata",
]) requireMarker(sources.tests, marker, files.tests);

if (failures.length > 0) {
  console.error("marketplace listing event contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace listing event contract verification passed");
