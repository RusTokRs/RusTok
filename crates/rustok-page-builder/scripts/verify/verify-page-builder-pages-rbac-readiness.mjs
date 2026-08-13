#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

const checks = [
  {
    file: "crates/rustok-pages/tests/rbac.rs",
    label: "rustok-pages RBAC regression coverage",
    tokens: [
      "manager_cannot_publish_during_create_or_non_builder_lifecycle_transition",
      "customer_reads_only_published_pages",
      "PagesError::Forbidden",
      "publish_non_builder_if_current",
      ".list(",
      "expect(\"public list\")",
    ],
  },
];

function fail(message) {
  console.error("[verify-page-builder-pages-rbac-readiness] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

for (const check of checks) {
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
  console.log(`[verify-page-builder-pages-rbac-readiness] ${check.label}: PASS`);
}

console.log("[verify-page-builder-pages-rbac-readiness] PASS");
