import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  types: "crates/rustok-events/src/types.rs",
  schema: "crates/rustok-events/src/schema.rs",
  tests: "crates/rustok-events/tests/canonical_contracts.rs",
  registry: "crates/rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json",
};

const events = [
  ["MarketplaceListingCreated", "marketplace.listing.created"],
  ["MarketplaceListingTermsUpdated", "marketplace.listing.terms_updated"],
  ["MarketplaceListingSubmittedForReview", "marketplace.listing.submitted_for_review"],
  ["MarketplaceListingApproved", "marketplace.listing.approved"],
  ["MarketplaceListingRejected", "marketplace.listing.rejected"],
  ["MarketplaceListingPublished", "marketplace.listing.published"],
  ["MarketplaceListingSuspended", "marketplace.listing.suspended"],
  ["MarketplaceListingReactivated", "marketplace.listing.reactivated"],
  ["MarketplaceListingArchived", "marketplace.listing.archived"],
];

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

const types = read(files.types);
const schema = read(files.schema);
const tests = read(files.tests);
const registrySource = read(files.registry);

for (const [variant, eventType] of events) {
  requireMarker(types, `${variant} {`, files.types);
  requireMarker(types, `Self::${variant} { .. }`, files.types);
  requireMarker(types, `"${eventType}"`, files.types);
  requireMarker(types, `Self::${variant} { .. } => 1`, files.types);
  requireMarker(schema, `event_type: "${eventType}"`, files.schema);
  requireMarker(tests, `DomainEvent::${variant} {`, files.tests);
  requireMarker(registrySource, `"${eventType}"`, files.registry);
}

for (const marker of [
  "validate_marketplace_listing_slug(\"market_slug\", market_slug)?",
  "validate_marketplace_listing_slug(\"channel_slug\", channel_slug)?",
  "validators::validate_not_nil_uuid(\"listing_id\", listing_id)?",
  "validators::validate_not_nil_uuid(\"seller_id\", seller_id)?",
  "validators::validate_not_nil_uuid(\"master_product_id\", master_product_id)?",
  "validators::validate_not_nil_uuid(\"master_variant_id\", master_variant_id)?",
  '"terms_version"',
]) requireMarker(types, marker, files.types);

for (const forbidden of ["note:", "reason:", "metadata:", "approval_note:", "suspension_reason:"]) {
  const marketplaceSection = types.slice(
    types.indexOf("// MARKETPLACE LISTING EVENTS"),
    types.indexOf("// INDEX EVENTS (CQRS)"),
  );
  forbidMarker(marketplaceSection, forbidden, files.types);
}

for (const marker of [
  "marketplace_listing_events_reject_noncanonical_scope_and_invalid_terms_version",
  "marketplace_listing_external_payload_excludes_owner_notes_and_metadata",
  '"note"',
  '"reason"',
  '"metadata"',
]) requireMarker(tests, marker, files.tests);

if (failures.length > 0) {
  console.error("marketplace listing event contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("marketplace listing event contract verification passed");
