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

const consumerManifest = read("crates/rustok-pages/rustok-module.toml");
const pagesPlan = read("crates/rustok-pages/docs/implementation-plan.md");
const leptosComposition = read("crates/rustok-pages/admin/src/composition.rs");

const removedNextAdminPaths = [
  "apps/next-admin/packages/blog/src/api/page-builder-errors.ts",
  "apps/next-admin/packages/blog/src/components/page-builder.tsx",
];
for (const relativePath of removedNextAdminPaths) {
  if (fs.existsSync(path.join(repoRoot, relativePath))) {
    fail(`deleted Next-admin Page Builder surface reappeared: ${relativePath}`);
  }
}

for (const token of [
  "deleted Next/GrapesJS page-builder route",
  "Pages admin owns",
]) {
  if (!pagesPlan.includes(token)) {
    fail(`Pages implementation plan missing Leptos-only boundary '${token}'`);
  }
}

for (const token of ["PageBuilderAdmin", "PagesBuilderFacade", "PageBuilderAdminHostContext"]) {
  if (!leptosComposition.includes(token)) {
    fail(`Leptos Pages admin composition missing '${token}'`);
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

console.log("[verify-page-builder-next-admin-parity] PASS (Pages Leptos boundary)");
