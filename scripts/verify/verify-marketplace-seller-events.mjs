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
  receipted: "crates/rustok-marketplace-seller/src/receipted_commands.rs",
  lifecycleTests: "crates/rustok-marketplace-seller/src/seller_events_tests.rs",
  sellerResponseTests: "crates/rustok-marketplace-seller/src/seller_response_events_tests.rs",
  memberTests: "crates/rustok-marketplace-seller/src/seller_member_events_tests.rs",
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
  "append_receipted_member_event",
  '"create_seller"',
  '"update_seller_profile"',
  '"submit_seller_onboarding"',
  '"review_seller_onboarding"',
  '"suspend_seller"',
  '"reactivate_seller"',
  '"add_seller_member"',
  '"update_seller_member"',
  "MarketplaceSellerEventKind::Created",
  "MarketplaceSellerEventKind::ProfileUpdated",
  "MarketplaceSellerEventKind::OnboardingSubmitted",
  "MarketplaceSellerEventKind::OnboardingApproved",
  "MarketplaceSellerEventKind::OnboardingRejected",
  "MarketplaceSellerEventKind::Suspended",
  "MarketplaceSellerEventKind::Reactivated",
  "MarketplaceSellerEventKind::MemberAdded",
  "MarketplaceSellerEventKind::MemberUpdated",
  "seller response has no immutable event mapping",
  "member response has no immutable event mapping",
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
  "self.add_member_with_receipt(",
  "self.update_member_with_receipt(",
  "context.locale.as_str()",
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
    `${files.receipts}: immutable seller event must be appended before receipt completion`,
  );
}

for (const marker of [
  '"locale": locale',
  "append_receipted_member_event(",
  "finish_member_command(receipt, locale.as_str(), result).await",
  "rollback_command(receipt, error).await",
  "complete_command(receipt, RESPONSE_KIND_MEMBER, &response).await",
]) requireMarker(sources.receipted, marker, files.receipted);
const memberAppendIndex = sources.receipted.indexOf("append_receipted_member_event(");
const memberCompleteIndex = sources.receipted.indexOf(
  "complete_command(receipt, RESPONSE_KIND_MEMBER, &response).await",
);
if (
  memberAppendIndex < 0 ||
  memberCompleteIndex < 0 ||
  memberAppendIndex > memberCompleteIndex
) {
  failures.push(
    `${files.receipted}: immutable member event must be appended before receipt completion`,
  );
}

for (const marker of [
  "seller_event_timeline_is_bounded_newest_first_and_tenant_scoped",
  "seller_event_attribution_constraint_accepts_truthful_provenance_only",
  "lifecycle_commands_commit_one_event_with_state_and_receipt",
  "event_insert_failure_rolls_back_state_and_pending_receipt",
  "replay must not append duplicate events",
  "pending receipt must roll back with state",
]) requireMarker(sources.lifecycleTests, marker, files.lifecycleTests);
for (const marker of [
  "create_profile_and_submit_commit_one_event_per_receipt",
  "completed replay must not duplicate events",
  "MarketplaceSellerEventKind::Created",
  "MarketplaceSellerEventKind::ProfileUpdated",
  "MarketplaceSellerEventKind::OnboardingSubmitted",
]) requireMarker(
  sources.sellerResponseTests,
  marker,
  files.sellerResponseTests,
);
for (const marker of [
  "member_commands_commit_one_event_per_receipt_and_bind_locale",
  "replay must not duplicate member events",
  "MarketplaceSellerEventKind::MemberAdded",
  "MarketplaceSellerEventKind::MemberUpdated",
  "IdempotencyConflict",
]) requireMarker(sources.memberTests, marker, files.memberTests);

for (const marker of [
  '"table": "marketplace_seller_events"',
  '"status": "all_live_commands_atomic"',
  '"create_seller"',
  '"update_seller_profile"',
  '"submit_seller_onboarding"',
  '"review_seller_onboarding"',
  '"suspend_seller"',
  '"reactivate_seller"',
  '"add_seller_member"',
  '"update_seller_member"',
  '"compatibility_columns_removed": false',
  '"list_seller_events"',
]) requireMarker(sources.registry, marker, files.registry);

if (failures.length > 0) {
  console.error("marketplace seller event storage verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace seller event storage verification passed");
