#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

const files = {
  service: "crates/rustok-pages/src/services/page.rs",
  roundtripTest: "crates/rustok-pages/tests/page_builder_roundtrip.rs",
  adminCore: "crates/rustok-pages/admin/src/core.rs",
  adminUi: "crates/rustok-pages/admin/src/ui/leptos.rs",
  storefrontCore: "crates/rustok-pages/storefront/src/core.rs",
  storefrontUi: "crates/rustok-pages/storefront/src/ui/leptos.rs",
  moduleReadme: "crates/rustok-pages/README.md",
  localDocs: "crates/rustok-pages/docs/README.md",
  plan: "crates/rustok-pages/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
};

const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolutePath)) {
    failures.push(`${relativePath}: file is missing`);
    return "";
  }
  return fs.readFileSync(absolutePath, "utf8");
}

function assertContains(source, needle, description) {
  const ok = typeof needle === "string" ? source.includes(needle) : needle.test(source);
  if (!ok) failures.push(description);
}

function assertNotContains(source, needle, description) {
  const ok = typeof needle === "string" ? source.includes(needle) : needle.test(source);
  if (ok) failures.push(description);
}

const source = Object.fromEntries(Object.entries(files).map(([key, file]) => [key, read(file)]));

assertContains(
  source.service,
  "if let Some(blocks) = input.blocks",
  `${files.service}: create path must still accept initial legacy blocks as import/bridge payload`,
);
assertContains(
  source.service,
  "BlockService::create_in_tx",
  `${files.service}: create path must keep block creation delegated to BlockService`,
);
assertContains(
  source.service,
  "self.upsert_body_in_tx(&txn, page_id, body, now).await?;",
  `${files.service}: visual-builder body upsert must remain a separate write path`,
);
assertNotContains(
  source.service,
  /delete_all_for_page_in_tx\([\s\S]{0,220}upsert_body_in_tx/,
  `${files.service}: body writes must not delete legacy blocks`,
);
assertNotContains(
  source.service,
  /struct UpdatePageInput[\s\S]{0,320}blocks/,
  `${files.service}: update DTO must not gain a block write surface through visual-builder updates`,
);

for (const marker of [
  "legacy_block_driven_page_round_trips_without_body",
  "grapesjs_body_update_preserves_legacy_blocks",
  "legacy block-driven pages must not synthesize a body",
  "page should accept grapesjs body without deleting legacy blocks",
]) {
  assertContains(source.roundtripTest, marker, `${files.roundtripTest}: missing legacy bridge regression marker '${marker}'`);
}

for (const marker of ["existing_blocks", "Existing blocks", "not deleted automatically by grapesjs_v1 writes"]) {
  assertContains(source.adminCore + source.adminUi, marker, `admin pages UI/core must expose existing-block compatibility marker '${marker}'`);
}
for (const marker of ["summarize_legacy_blocks", "No page body or legacy blocks yet", "Legacy blocks are still attached"]) {
  assertContains(source.storefrontCore + source.storefrontUi, marker, `storefront pages UI/core must expose read-only legacy block marker '${marker}'`);
}

for (const [docKey, docPath] of [
  ["moduleReadme", files.moduleReadme],
  ["localDocs", files.localDocs],
  ["plan", files.plan],
  ["registry", files.registry],
]) {
  assertContains(source[docKey], "legacy", `${docPath}: must document the legacy blocks bridge`);
}
assertContains(source.plan, "Legacy blocks path работает в режиме read/bridge", `${files.plan}: hand-off acceptance must mark/read legacy bridge status`);
assertContains(source.plan, "verify-page-builder-pages-legacy-bridge.mjs", `${files.plan}: plan must mention this no-compile legacy bridge guardrail`);
assertContains(source.registry, "verify-page-builder-pages-legacy-bridge.mjs", `${files.registry}: readiness board must mention this no-compile legacy bridge guardrail`);

if (failures.length > 0) {
  console.error("[verify-page-builder-pages-legacy-bridge] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[verify-page-builder-pages-legacy-bridge] PASS");
