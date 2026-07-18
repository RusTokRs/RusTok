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
  commands: "crates/rustok-marketplace-seller/src/receipted_commands.rs",
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
  "belongs_to = \"super::seller::Entity\"",
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
  "order_by_desc(seller_event::Column::CreatedAt)",
  "order_by_desc(seller_event::Column::Id)",
  "MarketplaceSellerEventProvenance::Command",
  "MarketplaceSellerEventProvenance::LegacySnapshot",
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
  "seller_event_timeline_is_bounded_newest_first_and_tenant_scoped",
  "seller_event_attribution_constraint_accepts_truthful_provenance_only",
  "command event without actor must fail DB CHECK",
]) requireMarker(sources.tests, marker, files.tests);

for (const marker of [
  '"table": "marketplace_seller_events"',
  '"status": "schema_and_read_port_ready_write_path_pending"',
  '"write_commands_atomic": false',
  '"compatibility_columns_removed": false',
  '"list_seller_events"',
]) requireMarker(sources.registry, marker, files.registry);

forbidMarker(
  sources.commands,
  "append_seller_event(",
  files.commands,
);

if (failures.length > 0) {
  console.error("marketplace seller event storage verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace seller event storage verification passed");
