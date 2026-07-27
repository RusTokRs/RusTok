import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  workspace: "Cargo.toml",
  adminHost: "apps/admin/Cargo.toml",
  permissions: "crates/rustok-api/src/permissions.rs",
  owner: "crates/rustok-marketplace-listing/src/lib.rs",
  ownerPorts: "crates/rustok-marketplace-listing/src/ports.rs",
  ownerGraphql: "crates/rustok-marketplace-listing/src/graphql.rs",
  sellerGraphql: "crates/rustok-marketplace-seller/src/graphql.rs",
  sellerPorts: "crates/rustok-marketplace-seller/src/ports.rs",
  sellerManifest: "crates/rustok-marketplace-seller/rustok-module.toml",
  apiRuntime: "crates/rustok-api/src/runtime.rs",
  manifest: "crates/rustok-marketplace-listing/rustok-module.toml",
  serverRuntime: "apps/server/src/services/commerce_provider_runtime.rs",
  serverManifest: "apps/server/Cargo.toml",
  model: "crates/rustok-marketplace-listing/admin/src/model.rs",
  transport: "crates/rustok-marketplace-listing/admin/src/transport.rs",
  native: "crates/rustok-marketplace-listing/admin/src/transport/native_server_adapter.rs",
  graphql: "crates/rustok-marketplace-listing/admin/src/transport/graphql_adapter.rs",
  ui: "crates/rustok-marketplace-listing/admin/src/ui/leptos.rs",
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
const excludes = (source, marker, file) => {
  if (source.includes(marker)) failures.push(`${file}: forbidden ${marker}`);
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, file]) => [key, read(file)]),
);

contains(source.workspace, '"crates/rustok-marketplace-listing/admin"', files.workspace);
contains(
  source.workspace,
  'rustok-marketplace-listing-admin = { path = "crates/rustok-marketplace-listing/admin" }',
  files.workspace,
);
for (const marker of [
  "rustok-marketplace-listing-admin/hydrate",
  "rustok-marketplace-listing-admin/ssr",
  'rustok-marketplace-listing-admin = { path = "../../crates/rustok-marketplace-listing/admin"',
]) contains(source.adminHost, marker, files.adminHost);

for (const marker of [
  "MarketplaceListings",
  'Self::MarketplaceListings => "marketplace_listings"',
  '"marketplace_listings" => Ok(Self::MarketplaceListings)',
  "MARKETPLACE_LISTINGS_CREATE",
  "MARKETPLACE_LISTINGS_READ",
  "MARKETPLACE_LISTINGS_UPDATE",
  "MARKETPLACE_LISTINGS_LIST",
  "MARKETPLACE_LISTINGS_MANAGE",
  "MARKETPLACE_LISTINGS_PUBLISH",
  "MARKETPLACE_LISTINGS_MODERATE",
]) contains(source.permissions, marker, files.permissions);
for (const marker of [
  "MARKETPLACE_LISTINGS_CREATE",
  "MARKETPLACE_LISTINGS_READ",
  "MARKETPLACE_LISTINGS_UPDATE",
  "MARKETPLACE_LISTINGS_LIST",
  "MARKETPLACE_LISTINGS_MANAGE",
  "MARKETPLACE_LISTINGS_PUBLISH",
  "MARKETPLACE_LISTINGS_MODERATE",
]) contains(source.owner, marker, files.owner);

for (const marker of [
  'leptos_crate = "rustok-marketplace-listing-admin"',
  'route_segment = "marketplace-listings"',
  'supported_locales = ["en", "ru"]',
  "[provides.graphql]",
  "graphql::MarketplaceListingQuery",
  "graphql::MarketplaceListingMutation",
  "graphql::graphql_runtime_data",
]) contains(source.manifest, marker, files.manifest);

for (const marker of [
  "MarketplaceListingAdminAction",
  "pub const fn permission",
  "MARKETPLACE_LISTINGS_LIST",
  "MARKETPLACE_LISTINGS_MODERATE",
  'self.provenance == "legacy_snapshot"',
]) contains(source.model, marker, files.model);

contains(source.transport, "execute_selected_transport", files.transport);
contains(source.transport, '"never falls back"', files.transport);

for (const marker of [
  "action.permission()",
  "use_context::<HostRuntimeContext>()",
  "shared_get::<rustok_marketplace_listing::MarketplaceListingRuntime>()",
  "leptos_axum::extract::<AuthContext>()",
  "leptos_axum::extract::<TenantContext>()",
  "leptos_axum::extract::<RequestContext>()",
  "has_effective_permission",
  "request.user_id != Some(auth.user_id)",
  'is_tenant_module_enabled(host.db(), tenant.id, "marketplace_listing")',
  "MarketplaceListingReadPort::list_listing_events",
  "MarketplaceListingCommandPort::archive_listing",
]) contains(source.native, marker, files.native);
for (const marker of [
  "DatabaseConnection",
  "MarketplaceListingService::new",
  "entities::",
]) excludes(source.native, marker, files.native);

for (const marker of [
  "marketplaceListings",
  "marketplaceListingEvents",
  "createMarketplaceListing",
  "updateMarketplaceListingTerms",
  "submitMarketplaceListingForReview",
  "reviewMarketplaceListing",
  "publishMarketplaceListing",
  "suspendMarketplaceListing",
  "reactivateMarketplaceListing",
  "archiveMarketplaceListing",
  "execute_graphql",
]) contains(source.graphql, marker, files.graphql);
excludes(source.graphql, "UNMOUNTED", files.graphql);

for (const marker of [
  "pub trait MarketplaceListingPorts",
  "pub struct MarketplaceListingRuntime",
  "Arc<dyn MarketplaceListingPorts>",
]) contains(source.ownerPorts, marker, files.ownerPorts);
for (const marker of [
  "pub struct MarketplaceListingQuery",
  "pub struct MarketplaceListingMutation",
  "MarketplaceListingReadPort::list_listings",
  "MarketplaceListingReadPort::list_listing_events",
  "MarketplaceListingCommandPort::create_listing",
  "MarketplaceListingCommandPort::archive_listing",
  "graphql_runtime_data",
  "RequestContext",
  "require_module_enabled(ctx, MODULE_SLUG).await",
]) contains(source.ownerGraphql, marker, files.ownerGraphql);
for (const marker of [
  "MarketplaceListingRuntime",
  "MarketplaceSellerRuntime",
  "shared_read_port",
  "ProductCatalogReadPort",
  "server.shared_insert(runtime.clone())",
]) contains(source.serverRuntime, marker, files.serverRuntime);
contains(
  source.serverManifest,
  "rustok-marketplace-listing/graphql",
  files.serverManifest,
);
contains(source.apiRuntime, "pub async fn is_tenant_module_enabled", files.apiRuntime);
contains(source.transport, "pub locale: Option<String>", files.transport);
contains(source.graphql, "tenant_slug,\n        locale,", files.graphql);
contains(
  source.ownerGraphql,
  "require_permissions(ctx, &[Permission::MARKETPLACE_LISTINGS_LIST]).await",
  files.ownerGraphql,
);
contains(source.sellerPorts, "pub struct MarketplaceSellerRuntime", files.sellerPorts);
contains(source.sellerGraphql, "graphql_runtime_data", files.sellerGraphql);
contains(
  source.sellerGraphql,
  "require_module_enabled(ctx, MODULE_SLUG).await",
  files.sellerGraphql,
);
excludes(source.sellerGraphql, "MarketplaceSellerService", files.sellerGraphql);
excludes(source.sellerGraphql, "sea_orm::DatabaseConnection", files.sellerGraphql);
contains(
  source.sellerManifest,
  'runtime_data_factory = "graphql::graphql_runtime_data"',
  files.sellerManifest,
);

for (const marker of [
  "pending_command",
  "Retry same command",
  "idempotency_key",
  "Immutable history",
  "has_unknown_attribution",
]) contains(source.ui, marker, files.ui);

if (failures.length > 0) {
  console.error("marketplace listing admin FFA verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace listing admin FFA verification passed");
