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

function verifyNoncePolicy(policy, file, label) {
  if (!policy) return;
  requireMarker(policy, "{nonce}", file);
  requireMarker(policy, "{connect_sources}", file);

  const script = directive(policy, "script-src");
  if (!script) {
    failures.push(`${file}: ${label} script-src directive not found`);
  } else {
    for (const forbidden of ["'unsafe-inline'", "'unsafe-eval'"]) {
      if (script.includes(forbidden)) {
        failures.push(`${file}: ${label} script-src contains ${forbidden}`);
      }
    }
    if (!script.includes("{nonce}")) {
      failures.push(`${file}: ${label} script-src does not carry the response nonce`);
    }
  }

  const style = directive(policy, "style-src");
  if (!style) {
    failures.push(`${file}: ${label} style-src directive not found`);
  } else {
    if (style.includes("'unsafe-inline'")) {
      failures.push(`${file}: ${label} style-src contains blanket unsafe-inline`);
    }
    if (!style.includes("{nonce}")) {
      failures.push(`${file}: ${label} style-src does not carry the response nonce`);
    }
  }

  if (!policy.includes("script-src-attr 'none'")) {
    failures.push(`${file}: ${label} policy does not block inline script attributes`);
  }
  if (!policy.includes("style-src-attr 'none'")) {
    failures.push(`${file}: ${label} policy must block inline style attributes`);
  }
  if (policy.includes("style-src-attr 'unsafe-inline'")) {
    failures.push(`${file}: ${label} policy restores inline style attributes`);
  }
  for (const forbidden of ["'unsafe-eval'", " http:"]) {
    if (policy.includes(forbidden)) {
      failures.push(`${file}: ${label} policy contains ${forbidden}`);
    }
  }
}

function verifyConnectionProfiles(source, secureName, developmentName, file) {
  const secureConnect = stringConstant(source, secureName, file);
  if (secureConnect) {
    if (secureConnect.includes(" ws:")) {
      failures.push(`${file}: production connect sources contain plaintext ws:`);
    }
    if (!secureConnect.includes(" wss:")) {
      failures.push(`${file}: production connect sources must retain wss:`);
    }
  }

  const developmentConnect = stringConstant(source, developmentName, file);
  if (developmentConnect && !developmentConnect.includes(" ws:")) {
    failures.push(`${file}: development connect sources must explicitly carry local ws:`);
  }
}

const headersFile = "apps/server/src/middleware/security_headers.rs";
const reportsFile = "apps/server/src/middleware/csp_reports.rs";
const middlewareFile = "apps/server/src/middleware/mod.rs";
const webFile = "crates/rustok-web/src/lib.rs";
const storefrontFile = "apps/storefront/src/lib.rs";
const appRouterFile = "apps/server/src/services/app_router.rs";
const standaloneAdminSecurityFile = "apps/admin/src/app/security.rs";
const standaloneAdminMainFile = "apps/admin/src/main.rs";
const standaloneAdminAuthFile = "apps/admin/src/app/auth_ssr.rs";
const standaloneAdminCargoFile = "apps/admin/Cargo.toml";
const inventoryFile = "docs/security/csp-report-only-inventory.md";

const headers = read(headersFile);
const reports = read(reportsFile);
const middleware = read(middlewareFile);
const web = read(webFile);
const storefront = read(storefrontFile);
const appRouter = read(appRouterFile);
const standaloneAdminSecurity = read(standaloneAdminSecurityFile);
const standaloneAdminMain = read(standaloneAdminMainFile);
const standaloneAdminAuth = read(standaloneAdminAuthFile);
const standaloneAdminCargo = read(standaloneAdminCargoFile);
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
  "style-src-attr 'none'",
  "plaintext_websocket_allowed()",
]) {
  requireMarker(headers, marker, headersFile);
}

verifyNoncePolicy(
  stringConstant(headers, "UI_CSP_TEMPLATE", headersFile),
  headersFile,
  "server-hosted UI",
);
verifyConnectionProfiles(
  headers,
  "SECURE_UI_CONNECT_SOURCES",
  "DEVELOPMENT_UI_CONNECT_SOURCES",
  headersFile,
);

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
  if (!directive(reportOnly, "style-src")?.includes("{nonce}")) {
    failures.push(`${headersFile}: report-only style-src does not carry the response nonce`);
  }
  if (!reportOnly.includes("style-src-attr 'none'")) {
    failures.push(`${headersFile}: report-only policy does not block inline style attributes`);
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
  "nonce_trusted_admin_elements",
  'html.replace("<script", script_opening.as_str())',
  '.replace("<style", style_opening.as_str())',
  "immutable bundled index document",
  "if !is_document",
]) {
  requireMarker(appRouter, marker, appRouterFile);
}

forbidMarker(headers, "style-src-attr 'unsafe-inline'", headersFile);
forbidMarker(standaloneAdminSecurity, "style-src-attr 'unsafe-inline'", standaloneAdminSecurityFile);

verifyNoncePolicy(
  stringConstant(standaloneAdminSecurity, "ADMIN_UI_CSP_TEMPLATE", standaloneAdminSecurityFile),
  standaloneAdminSecurityFile,
  "standalone admin UI",
);
verifyConnectionProfiles(
  standaloneAdminSecurity,
  "SECURE_CONNECT_SOURCES",
  "DEVELOPMENT_CONNECT_SOURCES",
  standaloneAdminSecurityFile,
);
for (const marker of [
  "pub async fn admin_security_headers",
  "request.extensions_mut().insert(nonce.clone())",
  "pub fn request_csp_nonce() -> Option<CspNonce>",
  "pub fn validate_admin_security_profile() -> Result<(), String>",
  "RUSTOK_HTTPS must be set to true",
  "intentionally emits no report-only endpoint",
]) {
  requireMarker(standaloneAdminSecurity, marker, standaloneAdminSecurityFile);
}
forbidMarker(
  standaloneAdminSecurity,
  "/api/security/csp-report",
  standaloneAdminSecurityFile,
);

for (const marker of [
  "validate_admin_security_profile()",
  "request_csp_nonce()",
  "provide_context(nonce)",
  ".layer(middleware::from_fn(admin_security_headers))",
]) {
  requireMarker(standaloneAdminMain, marker, standaloneAdminMainFile);
}

for (const marker of [
  "use rustok_web::CspNonce;",
  "or_else(crate::app::security::request_csp_nonce)",
  "nonce=nonce",
  'data-rustok-auth-bootstrap="local-storage-cookie-v1"',
]) {
  requireMarker(standaloneAdminAuth, marker, standaloneAdminAuthFile);
}

for (const marker of [
  '"dep:rustok-web"',
  "rustok-web = { workspace = true, optional = true }",
  "tower.workspace = true",
]) {
  requireMarker(standaloneAdminCargo, marker, standaloneAdminCargoFile);
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
  "## Trusted Script Nonce Boundary",
  "## Connection Profile Boundary",
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

console.log("✔ nonce-backed script/style elements, scoped style attributes, production WSS and bounded CSP collection are aligned");
