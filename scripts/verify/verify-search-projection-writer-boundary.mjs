#!/usr/bin/env node

/**
 * Search has one storage writer for owner projections, and it is module-neutral.
 *
 * `search_documents` is Search storage, but which rows exist is the owner's decision.
 * That is why the only Search-side writer must be `SearchProjectionDriver`, scoped by
 * `source.source_module()` and never by a hard-coded module name. Pages, Docs and Blog
 * were migrated onto the driver and their private copies were deleted; Content/Product
 * still composes its own statements, and it is listed below so the remaining debt is
 * explicit. A *new* private writer fails this guard instead of spreading silently, and
 * an allowlist entry that stops writing fails too: the list may only shrink.
 *
 * Run: `node scripts/verify/verify-search-projection-writer-boundary.mjs`
 */

import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);

const srcDir = "crates/modules/rustok-search/src";
const driverFile = "projection_driver.rs";
const read = (relative) => readFileSync(new URL(relative, root), "utf8");
const exists = (relative) => existsSync(new URL(relative, root));

/** Any statement that mutates the shared Search document table. */
const OWNER_WRITE =
  /(?:INSERT\s+INTO|DELETE\s+FROM|UPDATE)\s+search_documents\b/i;

/**
 * Files allowed to write `search_documents` outside the neutral driver.
 *
 * Each entry is named debt: route the owner through `SearchProjectionDriver`, then
 * delete the entry. Adding one requires a reviewable reason, because it makes a second
 * writer for one table.
 */
const remainingPrivateWriters = new Map([
  ["projector_core.rs", "Content/Product documents are composed here"],
]);

/** Files that must not come back: they were copies of the driver. */
const retiredProjectors = ["pages_projector.rs", "docs_projector.rs", "blog_projector.rs"];

/** Module names the neutral driver must never name. */
const moduleLiterals = ['"forum', '"blog', '"pages', '"docs', "forum_", "blog_", "pages_", "docs_"];

const failures = [];
const fail = (message) => failures.push(message);

const writableSources = readdirSync(new URL(srcDir, root), { withFileTypes: true })
  .filter((entry) => entry.isFile() && entry.name.endsWith(".rs"))
  .map((entry) => entry.name)
  .sort();

for (const name of writableSources) {
  if (name === driverFile) continue;
  const relative = `${srcDir}/${name}`;
  if (!OWNER_WRITE.test(read(relative))) continue;
  if (!remainingPrivateWriters.has(name)) {
    fail(
      `${relative} writes search_documents outside ${driverFile}. ` +
        `Route the owner through SearchProjectionDriver; if that is not possible yet, ` +
        `add a named entry to remainingPrivateWriters with the reason.`,
    );
  }
}

for (const [name, reason] of remainingPrivateWriters) {
  const relative = `${srcDir}/${name}`;
  if (!exists(relative)) {
    fail(`${relative} is gone, so its remainingPrivateWriters entry must be dropped`);
    continue;
  }
  if (!OWNER_WRITE.test(read(relative))) {
    fail(
      `${relative} no longer writes search_documents (${reason}); ` +
        `drop its remainingPrivateWriters entry so the allowlist stays shrinking`,
    );
  }
}

for (const name of retiredProjectors) {
  if (exists(`${srcDir}/${name}`)) {
    fail(`${srcDir}/${name} was replaced by ${driverFile} and must not exist`);
  }
}

const driver = read(`${srcDir}/${driverFile}`);
for (const marker of [
  "pub(crate) async fn rebuild_tenant(",
  "pub(crate) async fn refresh_entity(",
  "pub(crate) async fn delete_entity(",
  "pub(crate) async fn delete_tenant(",
  "fn validate_entity_type(",
  "document.document_id.is_nil()",
  "self.source.refresh_scope(entity_type)",
  "DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = $2",
  "self.source.source_module().into()",
]) {
  if (!driver.includes(marker)) {
    fail(`${srcDir}/${driverFile} is missing the storage contract marker: ${marker}`);
  }
}
for (const literal of moduleLiterals) {
  if (driver.includes(literal)) {
    fail(
      `${srcDir}/${driverFile} names a module (${literal}). The driver must stay ` +
        `module-neutral: scope everything by source.source_module().`,
    );
  }
}

const ingestion = read(`${srcDir}/ingestion.rs`);
for (const marker of [
  "fn build_projection_driver(",
  "fn owner_drivers(",
  "fn refresh_owner_entity(",
  "fn rebuild_owner_scope(",
  "fn delete_owner_entity(",
  "fn toggle_owner_scope(",
]) {
  if (!ingestion.includes(marker)) {
    fail(`${srcDir}/ingestion.rs is missing the neutral dispatch helper: ${marker}`);
  }
}
for (const retired of [
  "PagesSearchProjector",
  "DocsSearchProjector",
  "BlogSearchProjector",
]) {
  if (ingestion.includes(retired)) {
    fail(
      `${srcDir}/ingestion.rs still references ${retired}; the owner source must be ` +
        `driven through SearchProjectionDriver instead`,
    );
  }
}

if (failures.length > 0) {
  console.error("Search projection writer boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exitCode = 1;
} else {
  console.log(
    "Search owns one neutral projection writer; only the documented Content/Product " +
      "private statements remain.",
  );
}
