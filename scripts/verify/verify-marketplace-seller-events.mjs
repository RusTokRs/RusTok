import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  dto: "crates/rustok-marketplace-seller/src/dto.rs",
  entity: "crates/rustok-marketplace-seller/src/entities/seller_event.rs",
  entities: "crates/rustok-marketplace-seller/src/entities/mod.rs",
  migration: "crates/rustok-marketplace-seller/src/migrations/m20260718_000003_create_marketplace_seller_events.rs",
  migrations: "crates/rustok-marketplace-seller/src/migrations/mod.rs",
  reader: "crates/rustok-marketplace-seller/src/seller_events.rs",
  service: "crates/rustok-marketplace-seller/src/service.rs",
  ports: "crates/rustok-marketplace-seller/src/ports.rs",
  tests: "crates/rustok-marketplace-seller/src/seller_events_tests.rs",
  registry: "crates/rustok-marketplace-seller/contracts/marketplace-seller-fba-registry.json",
  receipts: "crates/rustok-marketplace-seller/src/command_receipts.rs",
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

const sources = Object.fromEntries(
  Object.entries(files).map(([key, file]) => [key, read(file)]),
);

for (const marker of [
  "MarketplaceSellerEventKind",
  "MarketplaceSellerEventProvenance",
  "MarketplaceSellerEventResponse",
  "ListMarketplaceSellerEventsRequest",
  "LegacyOnboardingSnapshot",
  "LegacySuspensionSnapshot",
]) requireMarker(sources.dto, marker, files.dto);

for (const marker of [
  'table_name = "marketplace_seller_events"',
  "pub actor_id: Option<Uuid>",
  "pub locale: Option<String>",
  "pub provenance: String",
  'belongs_to = "super::seller::Entity"',
]) requireMarker(sources.entity, marker, files.entity);
requireMarker(sources.entities, "pub mod seller_event;", files.entities);

for (const marker of [
  "MarketplaceSellerEvents::Table",
  "fk_marketplace_seller_events_tenant_seller",
  "idx_marketplace_seller_events_timeline",
  "idx_marketplace_seller_events_kind",
  "idx_marketplace_seller_events_actor",
  "legacy_snapshot",
  "actor_id IS NOT NULL",
  "locale IS NOT NULL",
]) requireMarker(sources.migration, marker, files.migration);
requireMarker(
  sources.migrations,
  "m20260718_000003_create_marketplace_seller_events",
  files.migrations,
);

for (const marker of [
  "MAX_EVENTS_PER_READ: u64 = 200",
  "list_seller_events",
  "append_receipted_seller_event",
  '"review_seller_onboarding" | "suspend_seller" | "reactivate_seller"',
  "MarketplaceSellerEventKind::OnboardingApproved",
  "MarketplaceSellerEventKind::OnboardingRejected",
  "MarketplaceSellerEventKind::Suspended",
  "MarketplaceSellerEventKind::Reactivated",
  "MarketplaceSellerEventProvenance::Command",
  "order_by_desc(seller_event::Column::CreatedAt)",
  "order_by_desc(seller_event::Column::Id)",
]) requireMarker(sources.reader, marker, files.reader);
for (const marker of [
  "pub async fn list_events(",
  "find_seller(&self.db, tenant_id, seller_id).await?",
  "list_seller_events(&self.db, tenant_id, seller_id, limit)",
]) requireMarker(sources.service, marker, files.service);
for (const marker of [
  "async fn list_seller_events(",
  "ListMarketplaceSellerEventsRequest",
  "MarketplaceSellerEventResponse",
  "PortCallPolicy::read()",
]) requireMarker(sources.ports, marker, files.ports);

for (const marker of [
  "pub actor_id: Uuid",
  "pub command_kind: String",
  "append_receipted_seller_event(",
  "receipt.transaction.rollback().await?",
  "seller_command_receipt::Entity::update_many()",
  "receipt.transaction.commit().await?",
]) requireMarker(sources.receipts, marker, files.receipts);
const appendIndex = sources.receipts.indexOf("append_receipted_seller_event(");
const receiptCompletionIndex = sources.receipts.indexOf(
  "seller_command_receipt::Entity::update_many()",
);
if (
  appendIndex < 0 ||
  receiptCompletionIndex < 0 ||
  appendIndex > receiptCompletionIndex
) {
  failures.push(
    `${files.receipts}: immutable event must be appended before receipt completion`,
  );
}

for (const marker of [
  "seller_event_timeline_is_bounded_newest_first_and_tenant_scoped",
  "seller_event_attribution_constraint_accepts_truthful_provenance_only",
  "lifecycle_commands_commit_one_event_with_state_and_receipt",
  "event_insert_failure_rolls_back_state_and_pending_receipt",
  "replay must not append duplicate events",
  "pending receipt must roll back with state",
]) requireMarker(sources.tests, marker, files.tests);

for (const marker of [
  '"table": "marketplace_seller_events"',
  '"status": "lifecycle_subset_atomic"',
  '"review_seller_onboarding"',
  '"suspend_seller"',
  '"reactivate_seller"',
  '"compatibility_columns_removed": false',
  '"list_seller_events"',
]) requireMarker(sources.registry, marker, files.registry);

if (failures.length > 0) {
  console.error("marketplace seller event storage verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace seller event storage verification passed");
