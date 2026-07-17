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
  sellerEntity: "crates/rustok-marketplace-seller/src/entities/seller.rs",
  sellerTranslationEntity: "crates/rustok-marketplace-seller/src/entities/seller_translation.rs",
  sellerLocalizedStorage: "crates/rustok-marketplace-seller/src/localized_sellers.rs",
  sellerDto: "crates/rustok-marketplace-seller/src/dto.rs",
  sellerService: "crates/rustok-marketplace-seller/src/service.rs",
  sellerPorts: "crates/rustok-marketplace-seller/src/ports.rs",
  sellerMigration: "crates/rustok-marketplace-seller/src/migrations/m20260716_000001_create_marketplace_sellers.rs",
  sellerReceiptMigration: "crates/rustok-marketplace-seller/src/migrations/m20260716_000002_create_seller_command_receipts.rs",
  sellerReceiptEntity: "crates/rustok-marketplace-seller/src/entities/seller_command_receipt.rs",
  sellerReceiptExecutor: "crates/rustok-marketplace-seller/src/command_receipts.rs",
  sellerReceiptedCommands: "crates/rustok-marketplace-seller/src/receipted_commands.rs",
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
const sellerEntity = read(files.sellerEntity);
const sellerTranslationEntity = read(files.sellerTranslationEntity);
const sellerLocalizedStorage = read(files.sellerLocalizedStorage);
const sellerDto = read(files.sellerDto);
const sellerService = read(files.sellerService);
const sellerPorts = read(files.sellerPorts);
const sellerMigration = read(files.sellerMigration);
const sellerReceiptMigration = read(files.sellerReceiptMigration);
const sellerReceiptEntity = read(files.sellerReceiptEntity);
const sellerReceiptExecutor = read(files.sellerReceiptExecutor);
const sellerReceiptedCommands = read(files.sellerReceiptedCommands);
const sellerAdminCore = read(files.sellerAdminCore);
const sellerAdminTransport = read(files.sellerAdminTransport);
const sellerAdminUi = read(files.sellerAdminUi);

for (const marker of [
  "rustok-marketplace",
  "rustok-marketplace-seller",
  "rustok-marketplace-seller-admin",
]) assertContains(workspace, marker, `${files.workspace}: missing ${marker}`);
for (const marker of [
  "marketplace_seller =",
  "marketplace =",
  'depends_on = ["marketplace_seller"]',
]) assertContains(modules, marker, `${files.modules}: missing ${marker}`);
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
assertContains(rootManifest, "[fba.consumer]", `${files.rootManifest}: root consumer contract missing`);
assertContains(rootRegistry, '"owns_tables": false', `${files.rootRegistry}: root non-ownership missing`);
assertContains(rootSource, "MARKETPLACE_FAMILY_MODULES", `${files.rootSource}: family descriptor missing`);
assertContains(rootConsumer, "Arc<dyn MarketplaceSellerReadPort>", `${files.rootConsumer}: typed seller consumer missing`);
assertNotContains(rootConsumer, "sea_orm", `${files.rootConsumer}: root consumer must not query seller storage`);
assertNotContains(rootConsumer, "entities::", `${files.rootConsumer}: root consumer must not import seller entities`);
if (fs.existsSync(path.join(root, "crates/rustok-marketplace/src/entities"))) failures.push("crates/rustok-marketplace/src/entities: family root must not own entities");
if (fs.existsSync(path.join(root, "crates/rustok-marketplace/src/migrations"))) failures.push("crates/rustok-marketplace/src/migrations: family root must not own migrations");

assertContains(sellerManifest, 'slug = "marketplace_seller"', `${files.sellerManifest}: seller slug missing`);
assertContains(sellerManifest, 'leptos_crate = "rustok-marketplace-seller-admin"', `${files.sellerManifest}: admin FFA package missing`);
assertContains(sellerRegistry, '"MarketplaceSellerReadPort"', `${files.sellerRegistry}: read port missing`);
assertContains(sellerRegistry, '"MarketplaceSellerCommandPort"', `${files.sellerRegistry}: command port missing`);
assertContains(sellerRegistry, '"idempotency_required": true', `${files.sellerRegistry}: command idempotency admission missing`);
assertContains(sellerRegistry, '"atomic_with_owner_write": true', `${files.sellerRegistry}: receipt atomicity missing`);
assertContains(sellerRegistry, "lost_response_replay_returns_saved_result", `${files.sellerRegistry}: lost-response replay case missing`);

for (const marker of [
  "marketplace_sellers",
  "marketplace_seller_translations",
  "marketplace_seller_members",
  "ux_marketplace_sellers_tenant_handle",
  "ux_marketplace_seller_translations_locale",
  "idx_marketplace_seller_translations_search",
  "fk_marketplace_seller_translations_tenant_seller",
  "ux_marketplace_seller_members_scope_user",
  "fk_marketplace_seller_members_tenant_seller",
  "MarketplaceSellerTranslations::Locale",
  ".string_len(32)",
]) assertContains(sellerMigration, marker, `${files.sellerMigration}: missing multilingual schema invariant ${marker}`);
assertContains(sellerTranslationEntity, 'table_name = "marketplace_seller_translations"', `${files.sellerTranslationEntity}: translation table ownership missing`);
assertContains(sellerTranslationEntity, "pub locale: String", `${files.sellerTranslationEntity}: locale column missing`);
assertContains(sellerTranslationEntity, "pub display_name: String", `${files.sellerTranslationEntity}: localized display name missing`);
assertNotContains(sellerEntity, "pub display_name:", `${files.sellerEntity}: localized display_name must not remain in base row`);
assertNotContains(sellerMigration, "MarketplaceSellers::DisplayName", `${files.sellerMigration}: base seller migration must not own localized display_name`);
assertContains(sellerDto, "pub resolved_locale: String", `${files.sellerDto}: resolved locale projection missing`);

for (const marker of [
  "marketplace_seller_command_receipts",
  "uq_marketplace_seller_command_receipt_key",
  "RequestHash",
  "ResponseJson",
  "CompletedAt",
]) {
  if (!sellerReceiptMigration.includes(marker) && !sellerReceiptEntity.includes(marker)) failures.push(`${files.sellerReceiptMigration}: missing receipt invariant ${marker}`);
}
for (const marker of [
  "normalize_locale_tag",
  "Column::Locale.eq(locale.as_str())",
  "OnConflict::columns",
  'update_column(Alias::new("display_name"))',
  "MISSING_TRANSLATION_PREFIX",
  "resolved_locale: translation.locale",
]) assertContains(sellerLocalizedStorage, marker, `${files.sellerLocalizedStorage}: missing exact-locale invariant ${marker}`);
assertNotContains(sellerLocalizedStorage, "build_locale_candidates", `${files.sellerLocalizedStorage}: owner must not invent locale fallback`);
assertNotContains(sellerLocalizedStorage, "PLATFORM_FALLBACK_LOCALE", `${files.sellerLocalizedStorage}: owner must not invent platform fallback`);
assertContains(sellerService, "localized_seller_ids_for_search", `${files.sellerService}: localized search boundary missing`);
assertContains(sellerService, "owner membership role cannot be changed", `${files.sellerService}: owner role invariant missing`);
assertContains(sellerService, "owner membership cannot be disabled", `${files.sellerService}: owner status invariant missing`);
for (const forbidden of [
  "pub async fn create_seller(",
  "pub async fn update_profile(",
  "pub async fn submit_onboarding(",
  "pub async fn review_onboarding(",
  "pub async fn suspend_seller(",
  "pub async fn reactivate_seller(",
]) assertNotContains(sellerService, forbidden, `${files.sellerService}: non-receipted write bypass remains: ${forbidden}`);

for (const marker of [
  "pub trait MarketplaceSellerReadPort",
  "pub trait MarketplaceSellerCommandPort",
  "PortCallPolicy::read()",
  "PortCallPolicy::write()",
  "context.locale.as_str()",
  "create_seller_with_receipt",
  "update_profile_with_receipt",
  "submit_onboarding_with_receipt",
  "review_onboarding_with_receipt",
  "suspend_seller_with_receipt",
  "reactivate_seller_with_receipt",
  "add_member_with_receipt",
  "update_member_with_receipt",
  "marketplace_seller.translation_missing",
  "marketplace seller storage is temporarily unavailable",
]) assertContains(sellerPorts, marker, `${files.sellerPorts}: missing localized FBA invariant ${marker}`);
assertNotContains(sellerPorts, "storage unavailable: {error}", `${files.sellerPorts}: storage internals must not be exposed`);
assertNotContains(sellerPorts, "self.create_seller(\n", `${files.sellerPorts}: create must use durable receipt path`);
assertNotContains(sellerPorts, "self.update_profile(\n", `${files.sellerPorts}: update must use durable receipt path`);

for (const marker of [
  "command_request_hash",
  "CommandReceiptAdmission",
  "RECEIPT_STATUS_COMPLETED",
  "response_json",
  "transaction.commit().await?",
  "IdempotencyConflict",
]) assertContains(sellerReceiptExecutor, marker, `${files.sellerReceiptExecutor}: missing receipt executor invariant ${marker}`);
for (const marker of [
  "create_seller_with_receipt",
  "update_profile_with_receipt",
  "submit_onboarding_with_receipt",
  "review_onboarding_with_receipt",
  "suspend_seller_with_receipt",
  "reactivate_seller_with_receipt",
  "add_member_with_receipt",
  "update_member_with_receipt",
  '"locale": locale',
  "upsert_translation(",
  "complete_command(receipt",
  "rollback_command(receipt",
  "let policy_input = input.clone()",
]) assertContains(sellerReceiptedCommands, marker, `${files.sellerReceiptedCommands}: missing localized receipted command invariant ${marker}`);

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
