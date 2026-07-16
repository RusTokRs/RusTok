import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  modules: "modules.toml",
  manifest: "crates/rustok-marketplace-listing/rustok-module.toml",
  registry: "crates/rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json",
  migration: "crates/rustok-marketplace-listing/src/migrations/m20260716_000001_create_marketplace_listings.rs",
  service: "crates/rustok-marketplace-listing/src/service.rs",
  receipt: "crates/rustok-marketplace-listing/src/command_receipts.rs",
  replaySafe: "crates/rustok-marketplace-listing/src/replay_safe_commands.rs",
  ports: "crates/rustok-marketplace-listing/src/ports.rs",
  rootManifest: "crates/rustok-marketplace/rustok-module.toml",
  rootRegistry: "crates/rustok-marketplace/contracts/marketplace-fba-registry.json",
  rootConsumer: "crates/rustok-marketplace/src/listing_directory.rs",
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
const assertContains = (source, marker, file) => {
  if (!source.includes(marker)) failures.push(`${file}: missing ${marker}`);
};
const assertNotContains = (source, marker, file) => {
  if (source.includes(marker)) failures.push(`${file}: forbidden ${marker}`);
};

const modules = read(files.modules);
const manifest = read(files.manifest);
const registry = read(files.registry);
const migration = read(files.migration);
const service = read(files.service);
const receipt = read(files.receipt);
const replaySafe = read(files.replaySafe);
const ports = read(files.ports);
const rootManifest = read(files.rootManifest);
const rootRegistry = read(files.rootRegistry);
const rootConsumer = read(files.rootConsumer);

for (const marker of [
  "marketplace_listing =",
  'depends_on = ["marketplace_seller", "product"]',
]) assertContains(modules, marker, files.modules);
const defaultEnabled = modules.split("default_enabled =")[1] ?? "";
assertNotContains(defaultEnabled, "marketplace_listing", files.modules);
assertNotContains(defaultEnabled, '"marketplace"', files.modules);

for (const marker of [
  'slug = "marketplace_listing"',
  "marketplace_seller",
  "product",
  "[fba.provider]",
  "marketplace-listing-fba-registry.json",
]) assertContains(manifest, marker, files.manifest);

for (const marker of [
  '"MarketplaceListingReadPort"',
  '"MarketplaceListingCommandPort"',
  '"canonical_product_content_copied": false',
  '"cross_module_foreign_keys": false',
  '"buy_box_ranking_owned": false',
  '"atomic_with_owner_write": true',
  "lost_response_replay_returns_saved_result",
]) assertContains(registry, marker, files.registry);

for (const marker of [
  "marketplace_listings",
  "marketplace_listing_terms",
  "marketplace_listing_command_receipts",
  "uq_marketplace_listings_scope",
  "uq_marketplace_listings_seller_sku",
  "uq_marketplace_listing_terms_version",
  "fk_marketplace_listing_terms_tenant_listing",
  "uq_marketplace_listing_command_receipt_key",
]) assertContains(migration, marker, files.migration);
for (const marker of [
  "fk_marketplace_listings_seller",
  "fk_marketplace_listings_product",
  "fk_marketplace_listing_terms_pricing",
  "fk_marketplace_listing_terms_inventory",
]) assertNotContains(migration, marker, files.migration);

for (const marker of [
  "Arc<dyn MarketplaceSellerReadPort>",
  "Arc<dyn ProductCatalogReadPort>",
  "read_variant_product_projection",
  "MarketplaceSellerStatus::Active",
  "current_terms_version",
  "listing_not_active",
  "listing_not_approved",
  "pricing_reference_missing",
  "inventory_reference_missing",
  "seller_not_active",
  "seller_unavailable",
  "order_by_asc(listing::Column::SellerId)",
]) assertContains(service, marker, files.service);
for (const marker of [
  "rustok_marketplace_seller::entities",
  "rustok_product::entities",
  "buy_box",
  "rank_score",
]) assertNotContains(service, marker, files.service);

for (const marker of [
  "canonical_json",
  "Sha256::digest",
  "replay_existing",
  "STATUS_COMPLETED",
  "transaction.commit().await?",
  "IdempotencyConflict",
]) assertContains(receipt, marker, files.receipt);
for (const marker of [
  "create_listing_replay_safe",
  "publish_listing_replay_safe",
  "reactivate_listing_replay_safe",
  "replay_existing",
]) assertContains(replaySafe, marker, files.replaySafe);
for (const marker of [
  "pub trait MarketplaceListingReadPort",
  "pub trait MarketplaceListingCommandPort",
  "create_listing_replay_safe",
  "publish_listing_replay_safe",
  "reactivate_listing_replay_safe",
  "marketplace listing storage is temporarily unavailable",
]) assertContains(ports, marker, files.ports);
assertNotContains(ports, "storage unavailable: {error}", files.ports);

assertContains(rootManifest, "marketplace_listing", files.rootManifest);
assertContains(rootRegistry, '"provider": "marketplace_listing"', files.rootRegistry);
assertContains(rootRegistry, '"list_eligibility"', files.rootRegistry);
assertContains(rootConsumer, "Arc<dyn MarketplaceListingReadPort>", files.rootConsumer);
assertContains(rootConsumer, "list_eligibility", files.rootConsumer);
assertNotContains(rootConsumer, "sea_orm", files.rootConsumer);
assertNotContains(rootConsumer, "entities::", files.rootConsumer);

if (failures.length > 0) {
  console.error("marketplace listing boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace listing boundary verification passed");
