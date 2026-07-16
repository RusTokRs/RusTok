import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  workspace: "Cargo.toml",
  modules: "modules.toml",
  ecommercePlan: "crates/rustok-commerce/docs/implementation-plan.md",
  rootManifest: "crates/rustok-marketplace/rustok-module.toml",
  rootRegistry: "crates/rustok-marketplace/contracts/marketplace-fba-registry.json",
  rootSource: "crates/rustok-marketplace/src/lib.rs",
  rootConsumer: "crates/rustok-marketplace/src/seller_directory.rs",
  sellerManifest: "crates/rustok-marketplace-seller/rustok-module.toml",
  sellerRegistry: "crates/rustok-marketplace-seller/contracts/marketplace-seller-fba-registry.json",
  sellerService: "crates/rustok-marketplace-seller/src/service.rs",
  sellerPorts: "crates/rustok-marketplace-seller/src/ports.rs",
  sellerMigration: "crates/rustok-marketplace-seller/src/migrations/m20260716_000001_create_marketplace_sellers.rs",
  sellerAdminCore: "crates/rustok-marketplace-seller/admin/src/core.rs",
  sellerAdminTransport: "crates/rustok-marketplace-seller/admin/src/transport.rs",
  sellerAdminUi: "crates/rustok-marketplace-seller/admin/src/ui/leptos.rs",
};

const failures = [];
const read = (file) => {
  const absolute = path.join(root, file);
  if (!fs.existsSync(absolute)) {
    failures.push(`${file}: file is missing`);
    return "";
  }
  return fs.readFileSync(absolute, "utf8");
};
const assertContains = (source, marker, message) => {
  if (!source.includes(marker)) failures.push(message);
};
const assertNotContains = (source, marker, message) => {
  if (source.includes(marker)) failures.push(message);
};

const workspace = read(files.workspace);
const modules = read(files.modules);
const ecommercePlan = read(files.ecommercePlan);
const rootManifest = read(files.rootManifest);
const rootRegistry = read(files.rootRegistry);
const rootSource = read(files.rootSource);
const rootConsumer = read(files.rootConsumer);
const sellerManifest = read(files.sellerManifest);
const sellerRegistry = read(files.sellerRegistry);
const sellerService = read(files.sellerService);
const sellerPorts = read(files.sellerPorts);
const sellerMigration = read(files.sellerMigration);
const sellerAdminCore = read(files.sellerAdminCore);
const sellerAdminTransport = read(files.sellerAdminTransport);
const sellerAdminUi = read(files.sellerAdminUi);

for (const marker of [
  "rustok-marketplace",
  "rustok-marketplace-seller",
  "rustok-marketplace-seller-admin",
]) {
  assertContains(workspace, marker, `${files.workspace}: missing ${marker}`);
}
for (const marker of [
  "marketplace_seller =",
  "marketplace =",
  'depends_on = ["marketplace_seller"]',
]) {
  assertContains(modules, marker, `${files.modules}: missing ${marker}`);
}
for (const forbidden of [
  "crates/rustok-seller",
  "crates/rustok-offer",
  "crates/rustok-listing",
  "crates/rustok-commission",
  "crates/rustok-ledger",
  "crates/rustok-payout",
]) {
  assertNotContains(workspace, forbidden, `${files.workspace}: generic marketplace crate forbidden: ${forbidden}`);
  assertNotContains(modules, forbidden, `${files.modules}: generic marketplace module forbidden: ${forbidden}`);
}

assertContains(ecommercePlan, "## Marketplace Family", `${files.ecommercePlan}: marketplace family section missing`);
assertContains(ecommercePlan, "rustok-marketplace-listing", `${files.ecommercePlan}: listing family name missing`);
assertContains(ecommercePlan, "Marketplace promotion gates", `${files.ecommercePlan}: FFA/FBA gates missing`);

assertContains(rootManifest, 'slug = "marketplace"', `${files.rootManifest}: root slug missing`);
assertContains(rootManifest, '[fba.consumer]', `${files.rootManifest}: root consumer contract missing`);
assertContains(rootRegistry, '"owns_tables": false', `${files.rootRegistry}: root non-ownership missing`);
assertContains(rootSource, "MARKETPLACE_FAMILY_MODULES", `${files.rootSource}: family descriptor missing`);
assertContains(rootConsumer, "Arc<dyn MarketplaceSellerReadPort>", `${files.rootConsumer}: typed seller consumer missing`);
assertNotContains(rootConsumer, "sea_orm", `${files.rootConsumer}: root consumer must not query seller storage`);
assertNotContains(rootConsumer, "entities::", `${files.rootConsumer}: root consumer must not import seller entities`);
if (fs.existsSync(path.join(root, "crates/rustok-marketplace/src/entities"))) {
  failures.push("crates/rustok-marketplace/src/entities: family root must not own entities");
}
if (fs.existsSync(path.join(root, "crates/rustok-marketplace/src/migrations"))) {
  failures.push("crates/rustok-marketplace/src/migrations: family root must not own migrations");
}

assertContains(sellerManifest, 'slug = "marketplace_seller"', `${files.sellerManifest}: seller slug missing`);
assertContains(sellerManifest, 'leptos_crate = "rustok-marketplace-seller-admin"', `${files.sellerManifest}: admin FFA package missing`);
assertContains(sellerRegistry, '"MarketplaceSellerReadPort"', `${files.sellerRegistry}: read port missing`);
assertContains(sellerRegistry, '"MarketplaceSellerCommandPort"', `${files.sellerRegistry}: command port missing`);
assertContains(sellerRegistry, '"idempotency_required": true', `${files.sellerRegistry}: command idempotency admission missing`);
assertContains(sellerRegistry, "durable command receipts are not yet implemented", `${files.sellerRegistry}: known idempotency gap must remain explicit`);

for (const marker of [
  "marketplace_sellers",
  "marketplace_seller_members",
  "ux_marketplace_sellers_tenant_handle",
  "ux_marketplace_seller_members_scope_user",
  "fk_marketplace_seller_members_tenant_seller",
]) {
  assertContains(sellerMigration, marker, `${files.sellerMigration}: missing schema invariant ${marker}`);
}
for (const marker of [
  "self.db.begin().await?",
  "MarketplaceSellerMemberRole::Owner",
  "owner membership role cannot be changed",
  "owner membership cannot be disabled",
  "MarketplaceSellerOnboardingStatus::Submitted",
  "MarketplaceSellerStatus::Suspended",
]) {
  assertContains(sellerService, marker, `${files.sellerService}: missing owner invariant ${marker}`);
}
for (const marker of [
  "pub trait MarketplaceSellerReadPort",
  "pub trait MarketplaceSellerCommandPort",
  "PortCallPolicy::read()",
  "PortCallPolicy::write()",
  "marketplace seller storage is temporarily unavailable",
]) {
  assertContains(sellerPorts, marker, `${files.sellerPorts}: missing FBA invariant ${marker}`);
}
assertNotContains(sellerPorts, "storage unavailable: {error}", `${files.sellerPorts}: storage internals must not be exposed`);

assertContains(sellerAdminCore, "MarketplaceSellerAdminTransportProfile", `${files.sellerAdminCore}: transport selection missing`);
assertContains(sellerAdminCore, "Graphql", `${files.sellerAdminCore}: GraphQL profile missing`);
assertContains(sellerAdminTransport, "never falls back", `${files.sellerAdminTransport}: explicit no-fallback policy missing`);
assertContains(sellerAdminTransport, "transport_unmounted", `${files.sellerAdminTransport}: unmounted state missing`);
assertContains(sellerAdminUi, "pub fn MarketplaceSellerAdmin()", `${files.sellerAdminUi}: Leptos adapter missing`);

if (failures.length > 0) {
  console.error("marketplace family boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace family boundary verification passed");
