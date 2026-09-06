#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");
const failures = [];
const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const featureBody = (manifest, feature, label) => {
  const match = manifest.match(new RegExp(`^${feature}\\s*=\\s*\\[(.*?)\\]`, "ms"));
  if (!match) {
    failures.push(`${label}: missing ${feature} feature`);
    return "";
  }
  return match[1];
};

const evidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-authenticated-authoring-route-source.json",
));
const assetEvidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-inline-edit-asset-delivery-source.json",
));
const storefrontCore = read("apps/storefront/src/modules/core.rs");
const storefrontCargo = read("apps/storefront/Cargo.toml");
const storefrontBuild = read("apps/storefront/build.rs");
const bootstrap = read("apps/storefront/public/assets/pages-inline-edit-bootstrap.js");
const builder = [
  read("apps/storefront/scripts/build-pages-inline-edit-client.mjs"),
  read("apps/storefront/scripts/build-wasm-client.mjs"),
].join("\n");
const auth = read("apps/server/src/middleware/auth_context.rs");
const serverCargo = read("apps/server/Cargo.toml");
const consumer = read("crates/rustok-pages/storefront/src/inline_edit.rs");
const canonicalPlan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/rustok-pages/docs/implementation-plan.md");
const packet = read(
  "docs/modules/pages-page-builder-authenticated-authoring-route-packet-2026-08-06.md",
);

if (evidence.format !== "pages_authenticated_authoring_route_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_authenticated_authoring_route_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "existing_storefront_module_route_owner_reused",
  "authoring_route_segment_is_pages_authoring",
  "authoring_page_registration_is_feature_gated",
  "pages_module_enablement_still_admits_route",
  "page_id_query_parameter_is_required",
  "locale_is_bound_from_route_context",
  "existing_pages_inline_surface_is_reused",
  "direct_user_principal_required_before_route_render",
  "non_nil_authenticated_session_required",
  "pages_update_permission_required_before_route_render",
  "bootstrap_and_commit_server_functions_share_admission",
  "owner_aware_pages_document_authorization_remains_downstream",
  "html_and_server_function_responses_are_private_no_store",
  "authoring_html_is_noindex_nofollow_noarchive",
  "global_nonce_backed_ui_csp_is_reused",
  "bootstrap_is_external_same_origin_module_script",
  "bootstrap_imports_client_only_when_authoring_root_exists",
  "authorization_proof_is_not_written_to_dom",
  "fixed_js_and_wasm_asset_paths_are_declared",
  "wasm_export_is_feature_and_target_gated",
  "ssr_shell_is_removed_before_client_mount",
  "client_only_codegen_excludes_optional_server_modules",
  "dedicated_client_artifact_builder_source_added",
  "client_builder_enables_only_pages_inline_edit_hydrate",
  "anonymous_pages_route_is_unchanged",
  "anonymous_default_csr_hydrate_ssr_profiles_remain_without_inline_edit",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "database_schema_changed",
  "graphql_schema_changed",
  "rest_mutation_changed",
  "public_pages_route_changed",
  "page_document_persistence_owner_changed",
  "publish_or_rollback_behavior_changed",
  "built_client_artifact_produced",
  "asset_delivery_observed",
  "authenticated_route_executed",
  "browser_inline_edit_observed",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}
if (assetEvidence.status !== "pages_inline_edit_asset_delivery_source_unvalidated") {
  failures.push(`linked asset evidence status mismatch: ${assetEvidence.status}`);
}
if (assetEvidence.source_contract?.generated_assets_are_embedded_in_server_binary !== true) {
  failures.push("linked asset evidence must retain binary embedding source");
}
if (assetEvidence.source_contract?.asset_http_delivery_observed !== false) {
  failures.push("linked asset evidence must not claim HTTP execution");
}

for (const marker of [
  'PAGES_AUTHORING_ROUTE_SEGMENT: &str = "pages-authoring"',
  'PAGES_AUTHORING_BOOTSTRAP_ASSET: &str = "/assets/pages-inline-edit-bootstrap.js"',
  '#[cfg(feature = "pages-inline-edit")]',
  "register_page(StorefrontPageRegistration",
  'module_slug: "pages"',
  "route_segment: PAGES_AUTHORING_ROUTE_SEGMENT",
  'context.query.get("page_id")',
  "PagesAuthenticatedInlineEditSurface",
  'id="pages-inline-edit-client-root"',
  'data-pages-page-id=page_id.clone()',
  'data-pages-locale=locale.clone()',
  'type="module"',
  "src=PAGES_AUTHORING_BOOTSTRAP_ASSET",
  '#[cfg(all(feature = "pages-inline-edit-hydrate", target_arch = "wasm32"))]',
  "start_pages_inline_edit_client",
  'get_element_by_id("pages-inline-edit-client-root")',
  'body.set_inner_html("")',
  "mount_to_body",
]) need(storefrontCore, marker, "storefront authoring route");
for (const forbidden of [
  "authorization_proof",
  "data-inline-proof",
  "page_body::ActiveModel",
]) forbid(storefrontCore, forbidden, "storefront authoring route");

for (const marker of [
  'crate-type = ["cdylib", "rlib"]',
  "pages-inline-edit-hydrate = [",
  '"dep:wasm-bindgen"',
  '"rustok-pages-storefront/hydrate"',
  'wasm-bindgen = { version = "0.2", optional = true }',
  '"HtmlElement"',
  "[package.metadata.pages-inline-edit-client]",
  'bootstrap-path = "/assets/pages-inline-edit-bootstrap.js"',
  'module-path = "/assets/pages-inline-edit/rustok_storefront.js"',
  'wasm-path = "/assets/pages-inline-edit/rustok_storefront_bg.wasm"',
  'export = "start_pages_inline_edit_client"',
]) need(storefrontCargo, marker, "storefront client feature contract");
for (const baseFeature of ["default", "csr", "hydrate", "ssr"]) {
  forbid(
    featureBody(storefrontCargo, baseFeature, "storefront host manifest"),
    "pages-inline-edit",
    `storefront ${baseFeature} profile`,
  );
}
for (const marker of [
  'std::env::var_os("CARGO_FEATURE_CSR").is_some()',
  'std::env::var_os("CARGO_FEATURE_HYDRATE").is_some()',
  'std::env::var_os("CARGO_FEATURE_SSR").is_none()',
  "if client_only",
  "empty_storefront_codegen()",
]) need(storefrontBuild, marker, "client-only storefront codegen");

for (const marker of [
  'const MODULE_PATH = "/assets/pages-inline-edit/rustok_storefront.js"',
  'const WASM_PATH = "/assets/pages-inline-edit/rustok_storefront_bg.wasm"',
  'document.getElementById("pages-inline-edit-client-root")',
  'root.dataset.pagesAuthoringRoute !== "true"',
  "await import(MODULE_PATH)",
  "await module.default(WASM_PATH)",
  "module.start_pages_inline_edit_client()",
]) need(bootstrap, marker, "authoring bootstrap asset");
forbid(bootstrap, "authorization_proof", "authoring bootstrap asset");

for (const marker of [
  '"pages-inline-edit-hydrate"',
  '"wasm32-unknown-unknown"',
  '"rustok_storefront.wasm"',
  '"--print-wasm-bindgen-version"',
  'readFileSync(path.join(repoRoot, "Cargo.lock"), "utf8")',
  '"--locked"',
  "RUSTOK_WASM_BINDGEN_BIN",
  'run(wasmBindgen, ["--version"], true)',
  'run(wasmBindgen, [',
  '"--target"',
  '"web"',
  '"--out-name"',
  '"rustok_storefront"',
  'process.env.CARGO_TARGET_DIR?.trim()',
  "renameSync(stagingRoot, targetRoot)",
  'pages-inline-edit-bootstrap.js',
]) need(builder, marker, "authoring client artifact builder");
forbid(builder, "pages-inline-edit,ssr", "authoring client artifact builder");

for (const marker of [
  'PAGES_AUTHORING_CACHE_CONTROL: &str = "private, no-store"',
  'PAGES_AUTHORING_ROBOTS_POLICY: &str = "noindex, nofollow, noarchive"',
  "is_pages_inline_authoring_surface",
  "is_pages_inline_authoring_server_fn",
  "current_user.principal_kind.is_direct_user()",
  "current_user.session_id.is_nil()",
  "Permission::PAGES_UPDATE",
  "has_effective_permission",
  "presented_credentials || pages_inline_authoring",
  "pages_inline_authoring_response",
  '"cache-control"',
  '"x-robots-tag"',
  '"/api/fn/pages/inline-edit/bootstrap"',
  '"/api/fn/pages/inline-edit/commit"',
]) need(auth, marker, "host authoring admission");
need(
  serverCargo,
  'pages-inline-edit = ["embed-storefront", "mod-pages", "rustok-storefront/pages-inline-edit"]',
  "server opt-in feature",
);
need(
  serverCargo,
  'pages-inline-edit-assets = ["pages-inline-edit", "rustok-pages/inline-edit-assets"]',
  "server asset handoff feature",
);
forbid(
  featureBody(serverCargo, "default", "server manifest"),
  "pages-inline-edit",
  "server default profile",
);

for (const marker of [
  'endpoint = "pages/inline-edit/bootstrap"',
  'endpoint = "pages/inline-edit/commit"',
  "load_inline_edit_document(",
  ".save_document(",
  "expected_revision: claims.revision_id.clone()",
]) need(consumer, marker, "downstream Pages owner");

for (const marker of [
  "authenticated-authoring-route-source-ready",
  "Authenticated authoring route: source-ready",
  "client artifact build and browser execution remain pending",
  "inline-edit-asset-delivery-source-ready",
]) need(canonicalPlan, marker, "canonical Pages/Page Builder plan");
for (const marker of [
  "authenticated authoring route and shell: source-ready",
  "private, no-store",
  "client artifact build and browser execution remain pending",
  "authenticated route mount remains open",
  "inline edit asset delivery: source-ready",
]) need(localPlan, marker, "Pages local plan");
for (const marker of [
  "source-ready / execution-pending",
  "/modules/pages-authoring?page_id=",
  "direct authenticated user",
  "pages:update",
  "X-Robots-Tag",
  "pages-inline-edit-bootstrap.js",
  "build-pages-inline-edit-client.mjs",
  "artifact build and browser execution remain pending",
]) need(packet, marker, "authenticated authoring route packet");

if (failures.length > 0) {
  console.error("[verify-pages-authenticated-authoring-route] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-authenticated-authoring-route] PASS route_source_ready=true asset_delivery_source_ready=true execution=pending browser=pending",
);
