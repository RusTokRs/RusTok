#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

function fail(message) {
  console.error("[verify-page-builder-flutter-handoff] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

function read(relativePath) {
  const filePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(filePath)) fail(`missing file: ${relativePath}`);
  return fs.readFileSync(filePath, "utf8");
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

function requireArrayIncludes(array, expected, label) {
  if (!Array.isArray(array)) fail(`${label} must be an array`);
  for (const item of expected) {
    if (!array.includes(item)) fail(`${label} missing '${item}'`);
  }
}

const contract = readJson(
  "crates/rustok-page-builder/contracts/page-builder-flutter-wave-handoff.json",
);
const helper = read("rustok_mobile/packages/app_core/lib/src/page_builder_errors.dart");
const helperTest = read("rustok_mobile/packages/app_core/test/page_builder_errors_test.dart");
const centralPlan = read("docs/modules/page-builder-implementation-plan.md");
const flutterPlan = read("docs/research/flutter.md");

if (contract.artifact !== "page_builder_flutter_wave_handoff_contract") {
  fail(`unexpected artifact: ${contract.artifact}`);
}
if (contract.handoff_scope !== "device_runtime_evidence_only") {
  fail("handoff_scope must remain device_runtime_evidence_only");
}

requireArrayIncludes(contract.required_error_catalog, [
  "validation",
  "sanitize",
  "runtime",
  "feature-disabled",
], "required_error_catalog");
requireArrayIncludes(contract.required_stable_codes, ["FEATURE_DISABLED"], "required_stable_codes");
requireArrayIncludes(contract.required_toggle_guidance, [
  "builder.enabled",
  "builder.preview.enabled",
  "builder.properties.enabled",
  "builder.publish.enabled",
], "required_toggle_guidance");

for (const section of ["metadata", "profiles", "runtime_checks", "observability", "approvals"]) {
  if (!Array.isArray(contract.required_device_evidence_sections?.[section])) {
    fail(`required_device_evidence_sections.${section} must be an array`);
  }
}

for (const token of [
  "PageBuilderErrorCatalog",
  "PageBuilderErrorMapper",
  "validation",
  "sanitize",
  "runtime",
  "feature-disabled",
  "FEATURE_DISABLED",
  "operatorGuidance",
  "builder.publish.enabled",
]) {
  if (!helper.includes(token)) fail(`Flutter app_core helper missing '${token}'`);
}

for (const token of [
  "PageBuilderErrorCatalog.featureDisabled",
  "PageBuilderErrorCatalog.sanitize",
  "PageBuilderErrorCatalog.validation",
  "PageBuilderErrorCatalog.runtime",
]) {
  if (!helperTest.includes(token)) fail(`Flutter app_core tests missing '${token}'`);
}

for (const [label, text, tokens] of [
  ["central plan", centralPlan, ["Flutter device/runtime evidence"]],
  ["flutter research plan", flutterPlan, ["Flutter Wave hand-off must attach device/runtime evidence", "not duplicating"]],
]) {
  for (const token of tokens) {
    if (!text.includes(token)) fail(`${label} missing '${token}'`);
  }
}

for (const forbidden of contract.non_goals) {
  if (!forbidden || typeof forbidden !== "string") fail("non_goals must be non-empty strings");
}

console.log("[verify-page-builder-flutter-handoff] PASS");
