#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

function fail(message) {
  console.error("[verify-page-builder-next-admin-parity] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

function read(relativePath) {
  const filePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(filePath)) {
    fail(`missing file: ${relativePath}`);
  }
  return fs.readFileSync(filePath, "utf8");
}

const errorHelper = read(
  "apps/next-admin/src/features/blog/api/page-builder-errors.ts",
);
const pageBuilder = read(
  "apps/next-admin/src/features/blog/components/page-builder.tsx",
);
const consumerManifest = read("crates/rustok-pages/rustok-module.toml");

for (const token of [
  "validation",
  "sanitize",
  "runtime",
  "feature-disabled",
  "FEATURE_DISABLED",
  "resolvePageBuilderError",
  "operatorGuidance",
]) {
  if (!errorHelper.includes(token)) {
    fail(`Next admin page-builder error helper missing '${token}'`);
  }
}

for (const token of [
  "resolvePageBuilderError(error)",
  "setSaveError(viewModel)",
  "viewModel.operatorGuidance",
  "saveError.kind",
]) {
  if (!pageBuilder.includes(token)) {
    fail(`Next admin PageBuilder component missing '${token}'`);
  }
}

for (const token of [
  'feature_disabled = "feature-disabled"',
  'feature_disabled = "FEATURE_DISABLED"',
  'publish_disabled = "FEATURE_DISABLED"',
]) {
  if (!consumerManifest.includes(token)) {
    fail(`rustok-pages manifest missing '${token}' for Next parity`);
  }
}

console.log("[verify-page-builder-next-admin-parity] PASS");
