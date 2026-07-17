import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  lifecycle: "crates/rustok-marketplace-listing/src/lifecycle_event_commands.rs",
  moderation: "crates/rustok-marketplace-listing/src/evented_commands.rs",
  storage: "crates/rustok-marketplace-listing/src/listing_events.rs",
  ports: "crates/rustok-marketplace-listing/src/ports.rs",
  migration: "crates/rustok-marketplace-listing/src/migrations/m20260717_000002_create_marketplace_listing_events.rs",
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

const lifecycle = read(files.lifecycle);
const moderation = read(files.moderation);
const storage = read(files.storage);
const ports = read(files.ports);
const migration = read(files.migration);
const registry = read(files.registry);

for (const marker of [
  "update_terms_evented",
  "submit_for_review_evented",
  "archive_listing_evented",
  "MarketplaceListingEventKind::TermsUpdated",
  "MarketplaceListingEventKind::SubmittedForReview",
  "MarketplaceListingEventKind::Archived",
  '"locale": locale.clone()',
  "append_listing_event(",
  "complete(receipt, &response).await",
  "rollback(receipt, error).await",
]) requireMarker(lifecycle, marker, files.lifecycle);

for (const marker of [
  "review_listing_evented",
  "suspend_listing_evented",
  "MarketplaceListingEventKind::Approved",
  "MarketplaceListingEventKind::Rejected",
  "MarketplaceListingEventKind::Suspended",
  '"locale": locale.clone()',
  "append_listing_event(",
]) requireMarker(moderation, marker, files.moderation);

for (const marker of [
  "normalize_locale_tag",
  "limit.clamp(1, MAX_EVENTS_PER_READ)",
  "order_by_desc(listing_event::Column::CreatedAt)",
  "order_by_desc(listing_event::Column::Id)",
]) requireMarker(storage, marker, files.storage);

for (const marker of [
  "self.update_terms_evented(context, request)",
  "self.submit_for_review_evented(context, request.listing_id)",
  "self.review_listing_evented(context, request)",
  "self.suspend_listing_evented(context, request)",
  "self.archive_listing_evented(context, request.listing_id)",
]) requireMarker(ports, marker, files.ports);
for (const marker of [
  "self.update_terms(context, request)",
  "self.submit_for_review(context, request.listing_id)",
  "self.review_listing(context, request)",
  "self.suspend_listing(context, request)",
  "self.archive_listing(context, request.listing_id)",
]) forbidMarker(ports, marker, files.ports);

for (const marker of [
  "marketplace_listing_events",
  "fk_marketplace_listing_events_tenant_listing",
  "MarketplaceListingEvents::Locale",
  ".string_len(32)",
]) requireMarker(migration, marker, files.migration);

for (const marker of [
  '"event_table": "marketplace_listing_events"',
  '"locale_source": "PortContext.locale_effective"',
  '"append_only": true',
]) requireMarker(registry, marker, files.registry);

if (failures.length > 0) {
  console.error("marketplace listing lifecycle event verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace listing lifecycle event verification passed");
