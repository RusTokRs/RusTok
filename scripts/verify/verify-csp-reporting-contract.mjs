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

function stringConstant(source, name, file) {
  const match = new RegExp(`const ${name}: &str = "([^"]+)";`).exec(source);
  if (!match) {
    failures.push(`${file}: ${name} string constant not found`);
    return null;
  }
  return match[1];
}

function directive(policy, name) {
  return policy
    .split(";")
    .map((item) => item.trim())
    .find((item) => item.startsWith(name));
}

const headersFile = "apps/server/src/middleware/security_headers.rs";
const reportsFile = "apps/server/src/middleware/csp_reports.rs";
const middlewareFile = "apps/server/src/middleware/mod.rs";
const webFile = "crates/rustok-web/src/lib.rs";
const storefrontFile = "apps/storefront/src/lib.rs";
const appRouterFile = "apps/server/src/services/app_router.rs";
const inventoryFile = "docs/security/csp-report-only-inventory.md";

const headers = read(headersFile);
const reports = read(reportsFile);
const middleware = read(middlewareFile);
const web = read(webFile);
const storefront = read(storefrontFile);
const appRouter = read(appRouterFile);
const inventory = read(inventoryFile);

for (const marker of [
  "report-uri /api/security/csp-report",
  "report-to rustok-csp",
  "reporting-endpoints",
  "csp_reports::is_report_request",
  "csp_reports::handle(request).await",
  "CspNonce::generate",
  "request.extensions_mut().insert(nonce.clone())",
  "script-src-attr 'none'",
]) {
  requireMarker(headers, marker, headersFile);
}

const enforced = stringConstant(headers, "UI_CSP_TEMPLATE", headersFile);
if (enforced) {
  requireMarker(enforced, "{nonce}", headersFile);
  const script = directive(enforced, "script-src");
  if (!script) {
    failures.push(`${headersFile}: enforced script-src directive not found`);
  } else {
    for (const forbidden of ["'unsafe-inline'", "'unsafe-eval'"]) {
      if (script.includes(forbidden)) {
        failures.push(`${headersFile}: enforced script-src contains ${forbidden}`);
      }
    }
    if (!script.includes("{nonce}")) {
      failures.push(`${headersFile}: enforced script-src does not carry the response nonce`);
    }
  }
  for (const forbidden of ["'unsafe-eval'", " http:"]) {
    if (enforced.includes(forbidden)) {
      failures.push(`${headersFile}: enforced UI policy contains ${forbidden}`);
    }
  }
}

const reportOnly = stringConstant(headers, "UI_CSP_REPORT_ONLY_TEMPLATE", headersFile);
if (reportOnly) {
  for (const forbidden of ["'unsafe-inline'", "'unsafe-eval'", " http:", " ws:"]) {
    if (reportOnly.includes(forbidden)) {
      failures.push(`${headersFile}: report-only policy contains ${forbidden}`);
    }
  }
  if (!directive(reportOnly, "script-src")?.includes("{nonce}")) {
    failures.push(`${headersFile}: report-only script-src does not carry the response nonce`);
  }
}

for (const marker of [
  "pub struct CspNonce(String)",
  "Uuid::new_v4().simple().to_string()",
  "pub fn source_expression(&self) -> String",
]) {
  requireMarker(web, marker, webFile);
}

for (const marker of [
  'let trusted_opening_tag = r#"<script type="application/ld+json">"#',
  "nonce_structured_data_scripts",
  "Option<Extension<CspNonce>>",
  'assert!(rendered.contains("<script>alert(1)</script>"))',
]) {
  requireMarker(storefront, marker, storefrontFile);
}

for (const marker of [
  'request.extensions().get::<CspNonce>().cloned()',
  "nonce_trusted_admin_scripts",
  "immutable bundled index document",
  "if !is_document",
]) {
  requireMarker(appRouter, marker, appRouterFile);
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

console.log("✔ nonce-backed script CSP, trusted renderers and bounded violation collection are aligned");
