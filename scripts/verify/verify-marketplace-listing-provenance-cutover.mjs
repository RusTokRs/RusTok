import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  listing: "crates/rustok-marketplace-listing/src/entities/listing.rs",
  event: "crates/rustok-marketplace-listing/src/entities/listing_event.rs",
  dto: "crates/rustok-marketplace-listing/src/dto.rs",
  storage: "crates/rustok-marketplace-listing/src/listing_events.rs",
  migration: "crates/rustok-marketplace-listing/src/migrations/m20260717_000003_backfill_listing_event_provenance.rs",
  migrations: "crates/rustok-marketplace-listing/src/migrations/mod.rs",
  registry: "crates/rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json",
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

const listing = read(files.listing);
const event = read(files.event);
const dto = read(files.dto);
const storage = read(files.storage);
const migration = read(files.migration);
const migrations = read(files.migrations);
const registry = read(files.registry);

for (const marker of ["pub approval_note:", "pub suspension_reason:"]) {
  forbidMarker(listing, marker, files.listing);
  forbidMarker(dto, marker, files.dto);
}
for (const marker of [
  "pub actor_id: Option<Uuid>",
  "pub locale: Option<String>",
  "pub provenance: String",
]) requireMarker(event, marker, files.event);
for (const marker of [
  "MarketplaceListingEventProvenance",
  "LegacyApprovalSnapshot",
  "LegacySuspensionSnapshot",
  "pub actor_id: Option<Uuid>",
  "pub locale: Option<String>",
  "pub provenance: MarketplaceListingEventProvenance",
]) requireMarker(dto, marker, files.dto);
for (const marker of [
  "actor_id: Set(Some(actor_id))",
  "locale: Set(Some(locale))",
  "MarketplaceListingEventProvenance::Command",
  "command listing event is missing actor or locale attribution",
  "legacy listing snapshot must not fabricate actor or locale attribution",
]) requireMarker(storage, marker, files.storage);
for (const marker of [
  "ALTER COLUMN actor_id DROP NOT NULL",
  "ALTER COLUMN locale DROP NOT NULL",
  "ADD COLUMN provenance VARCHAR(32) NOT NULL DEFAULT 'command'",
  "ck_marketplace_listing_events_attribution",
  "legacy_approval_snapshot",
  "legacy_suspension_snapshot",
  '"original_actor_known": false',
  '"original_locale_known": false',
  "DROP COLUMN approval_note",
  "DROP COLUMN suspension_reason",
  "intentionally irreversible",
]) requireMarker(migration, marker, files.migration);
for (const marker of [
  "Uuid::nil()",
  "default_locale",
  "PLATFORM_FALLBACK_LOCALE",
]) forbidMarker(migration, marker, files.migration);
requireMarker(migrations, "m20260717_000003_backfill_listing_event_provenance", files.migrations);
for (const marker of [
  '"legacy_snapshot"',
  '"actor_id_must_be_null": true',
  '"locale_must_be_null": true',
  '"fabricated_attribution_forbidden": true',
  '"compatibility_snapshot_columns_removed"',
  "m20260717_000003_backfill_listing_event_provenance",
]) requireMarker(registry, marker, files.registry);

if (failures.length > 0) {
  console.error("marketplace listing provenance cutover verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace listing provenance cutover verification passed");
