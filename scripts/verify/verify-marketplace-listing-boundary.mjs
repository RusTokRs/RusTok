import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  modules: "modules.toml",
  manifest: "crates/rustok-marketplace-listing/rustok-module.toml",
  registry: "crates/rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json",
  lib: "crates/rustok-marketplace-listing/src/lib.rs",
  listingEntity: "crates/rustok-marketplace-listing/src/entities/listing.rs",
  termsEntity: "crates/rustok-marketplace-listing/src/entities/listing_terms.rs",
  eventEntity: "crates/rustok-marketplace-listing/src/entities/listing_event.rs",
  migration: "crates/rustok-marketplace-listing/src/migrations/m20260716_000001_create_marketplace_listings.rs",
  eventMigration: "crates/rustok-marketplace-listing/src/migrations/m20260717_000002_create_marketplace_listing_events.rs",
  provenanceMigration: "crates/rustok-marketplace-listing/src/migrations/m20260717_000003_backfill_listing_event_provenance.rs",
  service: "crates/rustok-marketplace-listing/src/service.rs",
  receipt: "crates/rustok-marketplace-listing/src/command_receipts.rs",
  replaySafe: "crates/rustok-marketplace-listing/src/replay_safe_commands.rs",
  ports: "crates/rustok-marketplace-listing/src/ports.rs",
  rootManifest: "crates/rustok-marketplace/rustok-module.toml",
  rootRegistry: "crates/rustok-marketplace/contracts/marketplace-fba-registry.json",
  rootConsumer: "crates/rustok-marketplace/src/listing_directory.rs",
  distributionManifest: "crates/rustok-distribution/Cargo.toml",
  distributionSource: "crates/rustok-distribution/src/lib.rs",
  serverManifest: "apps/server/Cargo.toml",
};

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

const sources = Object.fromEntries(Object.entries(files).map(([key, file]) => [key, read(file)]));

for (const marker of [
  "marketplace_listing =",
  'depends_on = ["marketplace_seller", "product"]',
]) requireMarker(sources.modules, marker, files.modules);
const defaultEnabled = sources.modules.split("default_enabled =")[1] ?? "";
forbidMarker(defaultEnabled, "marketplace_listing", files.modules);
forbidMarker(defaultEnabled, '"marketplace"', files.modules);

for (const marker of [
  'slug = "marketplace_listing"',
  "marketplace_seller",
  "product",
  "[fba.provider]",
  "marketplace-listing-fba-registry.json",
]) requireMarker(sources.manifest, marker, files.manifest);

for (const marker of [
  '"MarketplaceListingReadPort"',
  '"MarketplaceListingCommandPort"',
  '"event_table": "marketplace_listing_events"',
  '"compatibility_snapshot_columns_removed"',
  '"fabricated_attribution_forbidden": true',
  '"canonical_product_content_copied": false',
  '"localized_business_copy_owned": false',
  '"localized_business_copy_provider": "rustok-product"',
  '"cross_module_foreign_keys": false',
  '"buy_box_ranking_owned": false',
  '"direct_write_methods_in_service": false',
  '"atomic_with_owner_write": true',
  '"atomic_with_external_contract_event": true',
  '"event_bus_composition": "injected_through_marketplace_listing_service"',
  '"receipt_executor_constructs_transport": false',
  "replay_checked_before_provider_reads",
  "lost_response_replay_returns_saved_result",
]) requireMarker(sources.registry, marker, files.registry);

for (const marker of [
  "mod replay_safe_commands;",
  "mod evented_commands;",
  "mod lifecycle_event_commands;",
  "mod listing_events;",
]) requireMarker(sources.lib, marker, files.lib);

for (const marker of [
  "marketplace_listings",
  "marketplace_listing_terms",
  "MarketplaceListingCommandReceipts",
  "uq_marketplace_listings_scope",
  "uq_marketplace_listings_seller_sku",
  "uq_marketplace_listing_terms_version",
  "fk_marketplace_listing_terms_tenant_listing",
  "uq_marketplace_listing_command_receipt_key",
]) requireMarker(sources.migration, marker, files.migration);
for (const marker of [
  "marketplace_listing_events",
  "fk_marketplace_listing_events_tenant_listing",
  "MarketplaceListingEvents::Locale",
  ".string_len(32)",
]) requireMarker(sources.eventMigration, marker, files.eventMigration);
for (const marker of [
  "legacy_approval_snapshot",
  "legacy_suspension_snapshot",
  "DROP COLUMN approval_note",
  "DROP COLUMN suspension_reason",
]) requireMarker(sources.provenanceMigration, marker, files.provenanceMigration);

for (const marker of [
  "fk_marketplace_listings_seller",
  "fk_marketplace_listings_product",
  "fk_marketplace_listing_terms_pricing",
  "fk_marketplace_listing_terms_inventory",
]) forbidMarker(sources.migration, marker, files.migration);
for (const source of [sources.listingEntity, sources.termsEntity]) {
  for (const marker of [
    "pub title:",
    "pub description:",
    "pub localized_title:",
    "pub localized_description:",
    "pub translations_json:",
    "pub localized_fields_json:",
    "pub approval_note:",
    "pub suspension_reason:",
  ]) forbidMarker(source, marker, "marketplace listing entities");
}
for (const marker of [
  "pub actor_id: Option<Uuid>",
  "pub locale: Option<String>",
  "pub provenance: String",
]) requireMarker(sources.eventEntity, marker, files.eventEntity);

for (const marker of [
  "event_bus: TransactionalEventBus",
  "Arc<dyn MarketplaceSellerReadPort>",
  "Arc<dyn ProductCatalogReadPort>",
  "pub(crate) fn event_bus(&self) -> &TransactionalEventBus",
  "seller_reader(&self)",
  "product_reader(&self)",
  "listing_not_active",
  "listing_not_approved",
  "pricing_reference_missing",
  "inventory_reference_missing",
  "seller_not_active",
  "seller_unavailable",
  "order_by_asc(listing::Column::SellerId)",
]) requireMarker(sources.service, marker, files.service);
for (const marker of [
  "pub async fn create_listing(",
  "pub async fn update_terms(",
  "pub async fn submit_for_review(",
  "pub async fn review_listing(",
  "pub async fn publish_listing(",
  "pub async fn suspend_listing(",
  "pub async fn reactivate_listing(",
  "pub async fn archive_listing(",
  "rustok_marketplace_seller::entities",
  "rustok_product::entities",
  "OutboxTransport::new",
  "buy_box",
  "rank_score",
]) forbidMarker(sources.service, marker, files.service);

for (const marker of [
  "canonical_json",
  "Sha256::digest",
  "hex::encode",
  "replay_existing",
  "STATUS_COMPLETED",
  "event_bus: TransactionalEventBus",
  "publish_contract_in_tx(&transaction, tenant_id, Some(actor_id), event)",
  "transaction.commit().await?",
  "IdempotencyConflict",
]) requireMarker(sources.receipt, marker, files.receipt);
forbidMarker(sources.receipt, "OutboxTransport::new", files.receipt);
for (const marker of [
  "create_listing_replay_safe",
  "publish_listing_replay_safe",
  "reactivate_listing_replay_safe",
  "replay_existing(",
  "self.event_bus().clone()",
  "MarketplaceListingEventKind::Created",
  "MarketplaceListingEventKind::Published",
  "MarketplaceListingEventKind::Reactivated",
  "append_listing_event(",
]) requireMarker(sources.replaySafe, marker, files.replaySafe);
for (const marker of [
  "self.create_listing(context, input).await",
  "self.publish_listing(context, listing_id).await",
  "self.reactivate_listing(context, listing_id).await",
]) forbidMarker(sources.replaySafe, marker, files.replaySafe);

for (const marker of [
  "pub trait MarketplaceListingReadPort",
  "pub trait MarketplaceListingCommandPort",
  "list_listing_events",
  "create_listing_replay_safe",
  "update_terms_evented",
  "submit_for_review_evented",
  "review_listing_evented",
  "publish_listing_replay_safe",
  "suspend_listing_evented",
  "reactivate_listing_replay_safe",
  "archive_listing_evented",
  "marketplace listing storage is temporarily unavailable",
]) requireMarker(sources.ports, marker, files.ports);
forbidMarker(sources.ports, "storage unavailable: {error}", files.ports);

requireMarker(sources.rootManifest, "marketplace_listing", files.rootManifest);
requireMarker(sources.rootRegistry, '"provider": "marketplace_listing"', files.rootRegistry);
requireMarker(sources.rootRegistry, '"list_eligibility"', files.rootRegistry);
requireMarker(sources.rootConsumer, "Arc<dyn MarketplaceListingReadPort>", files.rootConsumer);
requireMarker(sources.rootConsumer, "list_eligibility", files.rootConsumer);
forbidMarker(sources.rootConsumer, "sea_orm", files.rootConsumer);
forbidMarker(sources.rootConsumer, "entities::", files.rootConsumer);

for (const marker of ["mod-marketplace_listing", "rustok-marketplace-listing"]) {
  requireMarker(sources.distributionManifest, marker, files.distributionManifest);
}
requireMarker(
  sources.distributionSource,
  "rustok_marketplace_listing::MarketplaceListingModule",
  files.distributionSource,
);
for (const marker of [
  "mod-marketplace_listing",
  "rustok-marketplace-listing",
  "rustok-distribution/mod-marketplace_listing",
]) requireMarker(sources.serverManifest, marker, files.serverManifest);
const serverDefaults = sources.serverManifest.split("default = [")[1]?.split("]")[0] ?? "";
forbidMarker(serverDefaults, "mod-marketplace_listing", files.serverManifest);
forbidMarker(serverDefaults, 'mod-marketplace"', files.serverManifest);

if (failures.length > 0) {
  console.error("marketplace listing boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace listing boundary verification passed");
