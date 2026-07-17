#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function requireMarker(source, marker, file) {
  if (!source.includes(marker)) {
    failures.push(`${file}: missing ${marker}`);
  }
}

function forbidMarker(source, marker, file) {
  if (source.includes(marker)) {
    failures.push(`${file}: forbidden ${marker}`);
  }
}

const headersFile = "apps/server/src/middleware/security_headers.rs";
const reportsFile = "apps/server/src/middleware/csp_reports.rs";
const middlewareFile = "apps/server/src/middleware/mod.rs";
const inventoryFile = "docs/security/csp-report-only-inventory.md";

const headers = read(headersFile);
const reports = read(reportsFile);
const middleware = read(middlewareFile);
const inventory = read(inventoryFile);

for (const marker of [
  "report-uri /api/security/csp-report",
  "report-to rustok-csp",
  "reporting-endpoints",
  "csp_reports::is_report_request",
  "csp_reports::handle(request).await",
]) {
  requireMarker(headers, marker, headersFile);
}

const enforcedMatch = /const UI_CSP: &str = "([^"]+)";/.exec(headers);
if (!enforcedMatch) {
  failures.push(`${headersFile}: enforced UI policy constant not found`);
} else {
  for (const forbidden of ["'unsafe-eval'", " http:"]) {
    if (enforcedMatch[1].includes(forbidden)) {
      failures.push(`${headersFile}: enforced UI policy contains ${forbidden}`);
    }
  }
}

const reportOnlyMatch = /const UI_CSP_REPORT_ONLY: &str = "([^"]+)";/.exec(headers);
if (!reportOnlyMatch) {
  failures.push(`${headersFile}: strict report-only policy constant not found`);
} else {
  for (const forbidden of ["'unsafe-inline'", "'unsafe-eval'", " http:", " ws:"]) {
    if (reportOnlyMatch[1].includes(forbidden)) {
      failures.push(`${headersFile}: report-only policy contains ${forbidden}`);
    }
  }
}

for (const marker of [
  'pub(crate) const CSP_REPORT_PATH: &str = "/api/security/csp-report"',
  "const MAX_CSP_REPORT_BYTES: usize = 64 * 1024",
  "const MAX_REPORTS_PER_REQUEST: usize = 20",
  "to_bytes(request.into_body(), MAX_CSP_REPORT_BYTES)",
  'value.get("csp-report")',
  'Some("csp-violation")',
  'format: "legacy"',
  'format: "reporting_api"',
  "normalized_directive",
  "sanitized_location",
  'record_module_error("security", directive, "warning")',
  'target: "rustok.security.csp"',
  'Some("opaque".to_string())',
]) {
  requireMarker(reports, marker, reportsFile);
}

for (const forbidden of [
  "script_sample",
  "script-sample:",
  "original_policy:",
  "query_pairs",
]) {
  forbidMarker(reports, forbidden, reportsFile);
}

requireMarker(middleware, "pub mod csp_reports;", middlewareFile);

for (const marker of [
  "## Collection Contract",
  "## Telemetry Contract",
  "## Target Policy Inventory",
  "## Current Migration Debt",
  "## Enforcement Exit Criteria",
  "64 KiB",
  "20 per request",
  "Never copy a full reported URL",
]) {
  requireMarker(inventory, marker, inventoryFile);
}

if (failures.length > 0) {
  console.error("CSP reporting contract verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ enforced/report-only CSP and bounded violation collection are aligned");
