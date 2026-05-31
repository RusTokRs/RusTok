#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

const docs = [
  "docs/modules/tiptap-page-builder-implementation-plan.md",
  "crates/rustok-pages/docs/implementation-plan.md",
  "crates/rustok-page-builder/docs/README.md",
];

const requiredTokens = [
  "Fallback matrix",
  "all_on",
  "publish_off",
  "preview_off",
  "builder_off",
  "typed_feature_disabled_error",
  "readonly_fallback",
  "stable",
];

function fail(message) {
  console.error("[verify-page-builder-fallback-matrix-docs] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

for (const doc of docs) {
  const fullPath = path.join(repoRoot, doc);
  if (!fs.existsSync(fullPath)) {
    fail(`missing file: ${doc}`);
  }

  const content = fs.readFileSync(fullPath, "utf8");
  for (const token of requiredTokens) {
    if (!content.includes(token)) {
      fail(`${doc} missing fallback matrix token '${token}'`);
    }
  }
}

console.log("[verify-page-builder-fallback-matrix-docs] PASS");
