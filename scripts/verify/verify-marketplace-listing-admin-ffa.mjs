import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  workspace: "Cargo.toml",
  adminHost: "apps/admin/Cargo.toml",
  permissions: "crates/rustok-api/src/permissions.rs",
  owner: "crates/rustok-marketplace-listing/src/lib.rs",
  manifest: "crates/rustok-marketplace-listing/rustok-module.toml",
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
  "MarketplaceListingAdminPorts",
  "MarketplaceListingAdminRequestScope",
  "action.permission()",
  "use_context::<MarketplaceListingAdminNativeRuntime>()",
  "marketplace listing native runtime is not mounted in this host",
  "MarketplaceListingReadPort::list_listing_events",
  "MarketplaceListingCommandPort::archive_listing",
]) contains(source.native, marker, files.native);
for (const marker of [
  "expect_context::<MarketplaceListingAdminNativeRuntime>",
  "DatabaseConnection",
  "MarketplaceListingService::new",
  "entities::",
]) excludes(source.native, marker, files.native);

contains(source.graphql, "declared_unmounted", files.graphql);
contains(
  source.graphql,
  "must provide module-owned listing queries and mutations",
  files.graphql,
);
excludes(source.graphql, "fallback", files.graphql);

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
