import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  manifest: "crates/rustok-marketplace-seller/rustok-module.toml",
  registry: "crates/rustok-marketplace-seller/contracts/marketplace-seller-fba-registry.json",
  ports: "crates/rustok-marketplace-seller/src/ports.rs",
  graphql: "crates/rustok-marketplace-seller/src/graphql.rs",
  adminModel: "crates/rustok-marketplace-seller/admin/src/model.rs",
  nativeAdapter: "crates/rustok-marketplace-seller/admin/src/transport/native_server_adapter.rs",
  graphqlAdapter: "crates/rustok-marketplace-seller/admin/src/transport/graphql_adapter.rs",
  transport: "crates/rustok-marketplace-seller/admin/src/transport.rs",
  ui: "crates/rustok-marketplace-seller/admin/src/ui/leptos.rs",
  distributionManifest: "crates/rustok-distribution/Cargo.toml",
  distributionSource: "crates/rustok-distribution/src/lib.rs",
  serverManifest: "apps/server/Cargo.toml",
  adminManifest: "apps/admin/Cargo.toml",
  modulesManifest: "modules.toml",
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
const contains = (source, marker, file) => {
  if (!source.includes(marker)) failures.push(`${file}: missing ${marker}`);
};
const forbids = (source, marker, file) => {
  if (source.includes(marker)) failures.push(`${file}: forbidden ${marker}`);
};

const manifest = read(files.manifest);
const registry = read(files.registry);
const ports = read(files.ports);
const graphql = read(files.graphql);
const adminModel = read(files.adminModel);
const nativeAdapter = read(files.nativeAdapter);
const graphqlAdapter = read(files.graphqlAdapter);
const transport = read(files.transport);
const ui = read(files.ui);
const distributionManifest = read(files.distributionManifest);
const distributionSource = read(files.distributionSource);
const serverManifest = read(files.serverManifest);
const adminManifest = read(files.adminManifest);
const modulesManifest = read(files.modulesManifest);

for (const marker of [
  "[provides.graphql]",
  "graphql::MarketplaceSellerQuery",
  "graphql::MarketplaceSellerMutation",
  "rustok-marketplace-seller-admin",
]) contains(manifest, marker, files.manifest);

for (const marker of [
  '"list_members"',
  '"atomic_with_owner_write": true',
  "native_and_graphql_share_command_envelope",
  "retry_reuses_original_idempotency_key",
]) contains(registry, marker, files.registry);

for (const marker of [
  "async fn list_members",
  "ListMarketplaceSellerMembersRequest",
  "context.locale.as_str()",
  "create_seller_with_receipt",
  "update_member_with_receipt",
]) contains(ports, marker, files.ports);

for (const marker of [
  "MarketplaceSellerReadPort::list_sellers",
  "MarketplaceSellerReadPort::list_members",
  "MarketplaceSellerCommandPort::create_seller",
  "MarketplaceSellerCommandPort::update_seller_member",
  "request::RequestContext",
  "request.locale.clone()",
  "resolved_locale: String",
  "resolved_locale: value.resolved_locale",
  "Permission::MARKETPLACE_SELLERS_MANAGE",
  "with_idempotency_key",
  "marketplace seller service is temporarily unavailable",
]) contains(graphql, marker, files.graphql);
forbids(graphql, "entities::", files.graphql);
forbids(graphql, "MarketplaceSellerService::list_members", files.graphql);
forbids(graphql, "storage unavailable: {error}", files.graphql);

for (const marker of [
  "marketplace_seller_directory_native",
  "marketplace_seller_detail_native",
  "marketplace_seller_command_native",
  "request::RequestContext",
  "request.locale.clone()",
  "request.channel_slug.clone()",
  "resolved_locale: seller.resolved_locale",
  "resolved_locale: value.resolved_locale",
  "MarketplaceSellerReadPort::list_members",
  "MarketplaceSellerCommandPort::create_seller",
  "ensure_permission",
  "ensure_tenant",
  "idempotency_key",
]) contains(nativeAdapter, marker, files.nativeAdapter);
forbids(nativeAdapter, "tenant.default_locale.clone()", files.nativeAdapter);
forbids(nativeAdapter, "entities::", files.nativeAdapter);

for (const marker of [
  "marketplaceSellers",
  "marketplaceSellerMembers",
  "createMarketplaceSeller",
  "updateMarketplaceSellerProfile",
  "submitMarketplaceSellerOnboarding",
  "reviewMarketplaceSellerOnboarding",
  "suspendMarketplaceSeller",
  "reactivateMarketplaceSeller",
  "addMarketplaceSellerMember",
  "updateMarketplaceSellerMember",
  "resolved_locale: resolvedLocale",
  "resolved_locale: String",
  "resolved_locale: item.resolved_locale",
  "resolved_locale: value.resolved_locale",
  "idempotencyKey",
]) contains(graphqlAdapter, marker, files.graphqlAdapter);

for (const marker of [
  "pub resolved_locale: String",
  "pub enum MarketplaceSellerAdminCommand",
  "ReviewOnboarding",
  "UpdateMember",
]) contains(adminModel, marker, files.adminModel);
for (const marker of [
  "execute_selected_transport",
  "MARKETPLACE_SELLER_TRANSPORT_FALLBACK_POLICY",
  "never falls back",
]) contains(transport, marker, files.transport);
for (const marker of [
  "pending_command",
  "Retry same command",
  "load_marketplace_seller_directory",
  "load_marketplace_seller_detail",
  "execute_marketplace_seller_command",
]) contains(ui, marker, files.ui);

for (const marker of ["mod-marketplace_seller", "mod-marketplace"]) {
  contains(distributionManifest, marker, files.distributionManifest);
}
contains(distributionSource, "rustok_marketplace_seller::MarketplaceSellerModule", files.distributionSource);
contains(distributionSource, "rustok_marketplace::MarketplaceModule", files.distributionSource);
contains(serverManifest, "rustok-marketplace-seller/graphql", files.serverManifest);
contains(serverManifest, "rustok-distribution/mod-marketplace_seller", files.serverManifest);
contains(adminManifest, "rustok-marketplace-seller-admin/hydrate", files.adminManifest);
contains(adminManifest, "rustok-marketplace-seller-admin/ssr", files.adminManifest);

const defaultEnabled = modulesManifest.split("default_enabled =")[1] ?? "";
if (defaultEnabled.includes("marketplace_seller") || defaultEnabled.includes('"marketplace"')) {
  failures.push("modules.toml: marketplace modules must not be default-enabled before runtime evidence");
}

if (failures.length > 0) {
  console.error("marketplace seller transport verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace seller transport verification passed");
