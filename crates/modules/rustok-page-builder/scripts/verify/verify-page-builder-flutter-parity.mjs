#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

function fail(message) {
  console.error("[verify-page-builder-flutter-parity] FAIL");
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

const helper = read(
  "rustok_mobile/packages/app_core/lib/src/page_builder_errors.dart",
);
const barrel = read("rustok_mobile/packages/app_core/lib/app_core.dart");
const testFile = read(
  "rustok_mobile/packages/app_core/test/page_builder_errors_test.dart",
);
const consumerManifest = read("crates/rustok-pages/rustok-module.toml");

for (const token of [
  "PageBuilderErrorCatalog",
  "validation",
  "sanitize",
  "runtime",
  "feature-disabled",
  "FEATURE_DISABLED",
  "PageBuilderErrorMapper",
  "operatorGuidance",
  "builder.publish.enabled",
]) {
  if (!helper.includes(token)) {
    fail(`Flutter app_core page-builder helper missing '${token}'`);
  }
}

if (!barrel.includes("page_builder_errors.dart")) {
  fail("app_core barrel does not export page_builder_errors.dart");
}

for (const token of [
  "PageBuilderErrorMapper",
  "PageBuilderErrorCatalog.featureDisabled",
  "PageBuilderErrorCatalog.sanitize",
  "PageBuilderErrorCatalog.validation",
  "PageBuilderErrorCatalog.runtime",
]) {
  if (!testFile.includes(token)) {
    fail(`Flutter page-builder parity test missing '${token}'`);
  }
}

for (const token of [
  'feature_disabled = "feature-disabled"',
  'feature_disabled = "FEATURE_DISABLED"',
  'publish_disabled = "FEATURE_DISABLED"',
]) {
  if (!consumerManifest.includes(token)) {
    fail(`rustok-pages manifest missing '${token}' for Flutter parity`);
  }
}

console.log("[verify-page-builder-flutter-parity] PASS");
