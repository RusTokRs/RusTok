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

const adminComposition = read("crates/rustok-pages/admin/src/composition.rs");
const adminBuilder = read("crates/rustok-pages/admin/src/builder.rs");
const rolloutSettings = read("crates/rustok-pages/admin/src/builder_rollout_settings.rs");
const sharedAdminUi = read("crates/rustok-page-builder/admin/src/ui/leptos.rs");
const consumerManifest = read("crates/rustok-pages/rustok-module.toml");

for (const token of [
  "PagesBuilderFacade",
  "PageBuilderAdminHostContext::new",
  "PageBuilderAdmin",
  "with_provider_status(provider_status)",
]) {
  if (!adminComposition.includes(token)) {
    fail(`Leptos Pages composition missing '${token}'`);
  }
}

for (const token of [
  "PageBuilderCapabilityRequest::Preview",
  "PageBuilderCapabilityRequest::Publish",
  "PageBuilderAdminFacadeError::with_stable_code",
  "PagesPageBuilderProjectStore",
]) {
  if (!adminBuilder.includes(token)) {
    fail(`Leptos Pages facade missing '${token}'`);
  }
}

for (const token of [
  "PageBuilderAdminProviderStatus::observed",
  "PageBuilderAdminProviderStatus::unobserved",
  "limit_capabilities",
]) {
  if (!rolloutSettings.includes(token)) {
    fail(`Leptos Pages rollout settings missing '${token}'`);
  }
}

for (const token of ["PageBuilderAdminHostContext", "editor_capability_evaluation"]) {
  if (!sharedAdminUi.includes(token)) {
    fail(`shared Leptos Page Builder UI missing '${token}'`);
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
