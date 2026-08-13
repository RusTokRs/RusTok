#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "..", "..", "..", "..");
const failures = [];

const files = {
  evidence:
    "crates/rustok-pages/contracts/evidence/pages-anonymous-storefront-ssr-delivery-source.json",
  graphEvidence:
    "crates/rustok-pages/contracts/evidence/pages-anonymous-storefront-graph-source.json",
  authoringEvidence:
    "crates/rustok-pages/contracts/evidence/pages-authenticated-authoring-route-source.json",
  graphVerifier:
    "crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs",
  inspector:
    "crates/rustok-pages/scripts/verify/inspect-pages-anonymous-storefront-ssr-artifact.mjs",
  packet:
    "docs/modules/pages-page-builder-anonymous-storefront-ssr-delivery-packet-2026-08-05.md",
  plan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
  cargo: "apps/storefront/Cargo.toml",
  host: "apps/storefront/src/lib.rs",
  authoringCore: "apps/storefront/src/modules/core.rs",
  regression: "apps/storefront/tests/pages_anonymous_ssr_delivery.rs",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => readFileSync(absolute(relativePath), "utf8");
const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const between = (text, start, end, label) => {
  const from = text.indexOf(start);
  if (from < 0) {
    failures.push(`${label}: missing start ${start}`);
    return "";
  }
  const to = text.indexOf(end, from + start.length);
  if (to < 0) {
    failures.push(`${label}: missing end ${end}`);
    return "";
  }
  return text.slice(from, to);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!existsSync(absolute(relativePath))) failures.push(`${label}: missing ${relativePath}`);
}
if (failures.length > 0) {
  console.error("[verify-pages-anonymous-storefront-ssr-delivery] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

const evidence = JSON.parse(read(files.evidence));
const graphEvidence = JSON.parse(read(files.graphEvidence));
const authoringEvidence = JSON.parse(read(files.authoringEvidence));
const cargo = read(files.cargo);
const host = read(files.host);
const authoringCore = read(files.authoringCore);
const regression = read(files.regression);
const inspector = read(files.inspector);
const packet = read(files.packet);
const plan = read(files.plan);

if (evidence.format !== "pages_anonymous_storefront_ssr_delivery_source_v1") {
  failures.push(`historical evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_anonymous_storefront_ssr_delivery_source_unvalidated") {
  failures.push(`historical evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("historical source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`historical validation.${key} must remain false`);
}
if (authoringEvidence.status !== "pages_authenticated_authoring_route_source_unvalidated") {
  failures.push(`authoring evidence status mismatch: ${authoringEvidence.status}`);
}
if (authoringEvidence.source_contract?.anonymous_pages_route_is_unchanged !== true) {
  failures.push("authoring evidence must retain unchanged anonymous Pages route");
}
if (
  authoringEvidence.source_contract
    ?.anonymous_default_csr_hydrate_ssr_profiles_remain_without_inline_edit !== true
) {
  failures.push("authoring evidence must retain anonymous profile exclusion");
}
if (authoringEvidence.source_contract?.built_client_artifact_produced !== false) {
  failures.push("authoring evidence must not claim a built client artifact");
}

if (graphEvidence.status !== "pages_anonymous_storefront_graph_source_unvalidated") {
  failures.push(`linked graph evidence status mismatch: ${graphEvidence.status}`);
}
if (graphEvidence.source_contract?.host_client_profiles_keep_optional_pages_module_disabled !== true) {
  failures.push("linked graph evidence must retain disabled Pages host client profiles");
}

for (const marker of [
  'crate-type = ["cdylib", "rlib"]',
  'csr = ["leptos/csr", "leptos_i18n/csr"]',
  'hydrate = ["leptos/hydrate", "leptos_i18n/hydrate"]',
  '"dep:rustok-pages-storefront"',
  '"rustok-pages-storefront/ssr"',
  'rustok-pages-storefront = { path = "../../crates/rustok-pages/storefront", default-features = false, optional = true }',
  "pages-inline-edit-hydrate = [",
  '"rustok-pages-storefront/hydrate"',
]) {
  need(cargo, marker, "storefront Cargo contract");
}
const csrFeature = between(cargo, 'csr = [', ']\nhydrate = [', "CSR feature");
const hydrateFeature = between(cargo, 'hydrate = [', ']\npages-inline-edit = [', "hydrate feature");
const ssrFeature = between(cargo, 'ssr = [', ']\n\n[dependencies]', "SSR feature");
for (const [label, feature] of [
  ["CSR", csrFeature],
  ["hydrate", hydrateFeature],
  ["SSR", ssrFeature],
]) {
  forbid(feature, "pages-inline-edit", `anonymous ${label} feature`);
  forbid(feature, "fly-leptos", `anonymous ${label} feature`);
}

const renderDocument = between(
  host,
  "fn render_document(",
  "#[cfg(feature = \"ssr\")]\nasync fn enabled_modules_or_empty",
  "SSR document renderer",
);
for (const marker of [
  '<link rel="stylesheet" href="/assets/app.css" />',
  '<div id="app">{app_html}</div>',
  "{extra_head}"
]) {
  need(renderDocument, marker, "SSR document renderer");
}
for (const marker of [
  "<script src=",
  'rel="modulepreload"',
  ".wasm",
  "/pkg/",
  "hydrate_body",
  "mount_to_body",
  "pages-inline-edit-bootstrap"
]) {
  forbid(renderDocument, marker, "anonymous SSR document renderer");
}
for (const marker of [
  'let comment_bootstrap = if app_html.contains',
  '.map(|nonce|',
  '/assets/blog-comment-bootstrap.js',
]) {
  need(renderDocument, marker, "gated Blog comment island bootstrap");
}
need(
  host,
  'render_document(locale, "RusToK Storefront", "", app_html, None)',
  "anonymous SSR shell invocation",
);

for (const marker of [
  '#[cfg(feature = "pages-inline-edit")]',
  'PAGES_AUTHORING_ROUTE_SEGMENT: &str = "pages-authoring"',
  'src=PAGES_AUTHORING_BOOTSTRAP_ASSET',
  '#[cfg(all(feature = "pages-inline-edit-hydrate", target_arch = "wasm32"))]',
  "start_pages_inline_edit_client",
  "mount_to_body(",
]) {
  need(authoringCore, marker, "gated authoring client source");
}
forbid(authoringCore, "#[wasm_bindgen(start)]", "gated authoring client source");
forbid(authoringCore, "wasm_bindgen(start)", "gated authoring client source");

function rustFiles(root) {
  const result = [];
  const pending = [absolute(root)];
  while (pending.length > 0) {
    const current = pending.pop();
    for (const entry of readdirSync(current)) {
      const item = path.join(current, entry);
      const stat = statSync(item);
      if (stat.isDirectory()) pending.push(item);
      else if (entry.endsWith(".rs")) result.push(item);
    }
  }
  return result;
}
for (const file of rustFiles("apps/storefront/src")) {
  const label = path.relative(repoRoot, file);
  if (label === files.authoringCore) continue;
  const source = readFileSync(file, "utf8");
  for (const marker of [
    "#[wasm_bindgen(start)]",
    "wasm_bindgen(start)",
    "mount_to_body(",
    "hydrate_body(",
    "leptos::mount::mount_to_body",
    "leptos::mount::hydrate_body"
  ]) {
    forbid(source, marker, label);
  }
}

for (const marker of [
  "const HOST_SOURCE: &str = include_str!(\"../src/lib.rs\")",
  "fn render_document_source()",
  "anonymous_pages_host_source_has_no_executable_client_bootstrap",
  'document.contains("<!DOCTYPE html>")',
  'document.contains("<div id=\\\"app\\\">{app_html}</div>")',
  'document.contains("<link rel=\\\"stylesheet\\\" href=\\\"/assets/app.css\\\" />")',
  '"#[wasm_bindgen(start)]"',
  '"rustok-pages-admin"',
  '"rustok-page-builder-admin"',
  '"PagesFlyBuilder"',
  '"PageBuilderAdmin"'
]) {
  need(regression, marker, "anonymous SSR source regression");
}

for (const marker of [
  "--artifact",
  "--output",
  "at least one --artifact is required",
  "createHash(\"sha256\")",
  "verify-pages-anonymous-storefront-graph.mjs",
  "absence_of_a_client_bundle_is_not_reported_as_a_passing_client_bundle",
  "forbidden_markers_found",
  "pages_anonymous_storefront_ssr_artifact_execution_v1"
]) {
  need(inspector, marker, "SSR artifact inspector");
}

for (const marker of [
  "source-ready / execution-pending",
  "SSR-only anonymous Pages delivery",
  "no executable client bootstrap",
  "explicit built artifact",
  "client bundle gate reopens",
  "Execution evidence remains pending"
]) {
  need(packet, marker, "SSR delivery packet");
}
for (const marker of [
  "anonymous-storefront-ssr-delivery-source-ready",
  "Anonymous storefront SSR delivery: source-ready",
  "public Pages route remains SSR-only",
  "authenticated inline profiles are opt-in",
  "client artifact build and browser execution remain pending"
]) {
  need(plan, marker, "canonical Pages/Page Builder plan");
}

if (failures.length > 0) {
  console.error("[verify-pages-anonymous-storefront-ssr-delivery] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-anonymous-storefront-ssr-delivery] PASS public_ssr_only=true authenticated_client=gated artifact_execution=pending",
);
