#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

function fail(message) {
  console.error("[verify-page-builder-leptos-admin-parity] FAIL");
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

const adminCore = read("crates/rustok-pages/admin/src/core.rs");
const adminLib = read("crates/rustok-pages/admin/src/lib.rs");
const enLocale = read("crates/rustok-pages/admin/locales/en.json");
const ruLocale = read("crates/rustok-pages/admin/locales/ru.json");
const consumerManifest = read("crates/rustok-pages/rustok-module.toml");

for (const token of [
  "issue_guidance",
  "is_builder_feature_disabled_issue",
  "feature-disabled",
  "builder.publish.enabled",
]) {
  if (!adminCore.includes(token)) {
    fail(`Leptos admin core missing '${token}'`);
  }
}

for (const token of [
  "pages.error.validationGuidance",
  "pages.error.sanitizeGuidance",
  "pages.error.runtimeGuidance",
  "pages.error.featureDisabledGuidance",
  "core::issue_guidance",
]) {
  if (!adminLib.includes(token)) {
    fail(`Leptos admin UI missing '${token}'`);
  }
}

for (const [label, content] of [
  ["en", enLocale],
  ["ru", ruLocale],
]) {
  for (const token of [
    "pages.error.validationGuidance",
    "pages.error.sanitizeGuidance",
    "pages.error.runtimeGuidance",
    "pages.error.featureDisabledGuidance",
  ]) {
    if (!content.includes(token)) {
      fail(`Leptos admin ${label} locale missing '${token}'`);
    }
  }
}

for (const token of [
  'feature_disabled = "feature-disabled"',
  'feature_disabled = "FEATURE_DISABLED"',
  'publish_disabled = "FEATURE_DISABLED"',
]) {
  if (!consumerManifest.includes(token)) {
    fail(`rustok-pages manifest missing '${token}' for Leptos parity`);
  }
}

console.log("[verify-page-builder-leptos-admin-parity] PASS");
