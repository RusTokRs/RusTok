#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

const pagesManifest = path.join(repoRoot, "crates", "rustok-pages", "rustok-module.toml");

function fail(message) {
  console.error("[verify-page-builder-fallback-profiles] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

if (!fs.existsSync(pagesManifest)) {
  fail(`missing file: ${pagesManifest}`);
}

const content = fs.readFileSync(pagesManifest, "utf8");

const requiredBlocks = [
  "[fba.builder_consumer.degraded_modes]",
  "[fba.builder_consumer.toggle_profiles]",
];

for (const block of requiredBlocks) {
  if (!content.includes(block)) {
    fail(`missing required block: ${block}`);
  }
}

const requiredKeys = [
  "builder_disabled",
  "preview_disabled",
  "publish_disabled",
  "all_on",
  "publish_off",
  "preview_off",
  "builder_off",
];

for (const key of requiredKeys) {
  const re = new RegExp(`^\\s*${key}\\s*=`, "m");
  if (!re.test(content)) {
    fail(`missing required key: ${key}`);
  }
}

const requiredFlags = [
  "builder.enabled",
  "builder.preview.enabled",
  "builder.properties.enabled",
  "builder.publish.enabled",
];

for (const flag of requiredFlags) {
  if (!content.includes(flag)) {
    fail(`missing toggle flag in profiles: ${flag}`);
  }
}

if (!content.includes("typed_feature_disabled_error_keep_read_paths")) {
  fail("publish_disabled degraded mode must keep read paths with typed error");
}

console.log("[verify-page-builder-fallback-profiles] PASS");
