#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

function fail(message) {
  console.error("[verify-page-builder-runtime-fallback-gate] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

const sourceChecks = [
  {
    label: "runtime rollout fallback matrix",
    file: "crates/rustok-page-builder/src/rollout.rs",
    tokens: [
      "fallback_matrix",
      "all_on",
      "publish_off",
      "preview_off",
      "builder_off",
      "typed_feature_disabled_error",
      "readonly_fallback",
    ],
  },
  {
    label: "runtime feature-disabled error catalog",
    file: "crates/rustok-page-builder/src/service.rs",
    tokens: [
      "PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE",
      "PageBuilderErrorKind::FeatureDisabled",
      "stable_code",
    ],
  },
  {
    label: "provider registry fallback profiles",
    file: "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
    tokens: [
      "all_on",
      "publish_off",
      "preview_off",
      "builder_off",
      "FEATURE_DISABLED",
    ],
  },
];

for (const check of sourceChecks) {
  const filePath = path.join(repoRoot, check.file);
  if (!fs.existsSync(filePath)) {
    fail(`${check.label}: missing file ${check.file}`);
  }
  const content = fs.readFileSync(filePath, "utf8");
  for (const token of check.tokens) {
    if (!content.includes(token)) {
      fail(`${check.label}: ${check.file} missing token '${token}'`);
    }
  }
  console.log(`[verify-page-builder-runtime-fallback-gate] ${check.label}: PASS`);
}

console.log("[verify-page-builder-runtime-fallback-gate] PASS (no-compile source gate)");
