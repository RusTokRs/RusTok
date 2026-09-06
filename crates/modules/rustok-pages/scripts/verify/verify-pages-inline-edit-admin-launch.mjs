#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");
const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const featureBody = (manifest, feature, label) => {
  const escaped = feature.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = manifest.match(new RegExp(`^${escaped}\\s*=\\s*\\[(.*?)\\]`, "ms"));
  if (!match) {
    failures.push(`${label}: missing ${feature} feature`);
    return "";
  }
  return match[1];
};

const files = {
  evidence: "crates/rustok-pages/contracts/evidence/pages-inline-edit-admin-launch-source.json",
  pagesAdminCargo: "crates/rustok-pages/admin/Cargo.toml",
  appAdminCargo: "apps/admin/Cargo.toml",
  pagesAdminLib: "crates/rustok-pages/admin/src/lib.rs",
  launch: "crates/rustok-pages/admin/src/inline_edit_launch.rs",
  localPlan: "crates/rustok-pages/docs/implementation-plan.md",
  plan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
  packet: "docs/modules/pages-page-builder-inline-edit-admin-launch-packet-2026-08-06.md",
};
for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(path.join(root, relativePath))) failures.push(`${label}: missing ${relativePath}`);
}
if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-admin-launch] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

const evidence = JSON.parse(read(files.evidence));
const pagesAdminCargo = read(files.pagesAdminCargo);
const appAdminCargo = read(files.appAdminCargo);
const pagesAdminLib = read(files.pagesAdminLib);
const launch = read(files.launch);
const localPlan = read(files.localPlan);
const plan = read(files.plan);
const packet = read(files.packet);

if (evidence.format !== "pages_inline_edit_admin_launch_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_inline_edit_admin_launch_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "pages_admin_owns_launch_control",
  "launch_control_is_feature_gated",
  "launch_feature_is_non_default",
  "same_origin_compile_time_acknowledgement_required",
  "same_origin_acknowledgement_defaults_to_disabled",
  "selected_page_uuid_is_required",
  "nil_page_uuid_is_rejected",
  "existing_pages_admin_transport_is_reused",
  "selected_page_detail_is_reloaded_before_launch",
  "exact_translation_or_body_locale_is_used",
  "published_page_launch_is_hidden",
  "locale_is_bounded",
  "locale_is_form_urlencoded",
  "fixed_authoring_route_is_used",
  "direct_absolute_external_origin_is_not_accepted",
  "current_editor_role_capability_gates_visibility",
  "route_owner_permission_checks_remain_authoritative",
  "launch_opens_new_tab_with_noopener_noreferrer",
  "token_is_not_added_to_href",
  "grant_proof_is_not_added_to_href",
  "authorization_material_is_not_added_to_dom",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "anonymous_storefront_route_changed",
  "page_document_persistence_owner_changed",
  "database_schema_changed",
  "graphql_schema_changed",
  "rest_mutation_changed",
  "publish_or_rollback_behavior_changed",
  "admin_asset_build_integrated",
  "release_workflow_integrated",
  "production_docker_builder_integrated",
  "launch_render_observed",
  "authenticated_route_navigation_observed",
  "browser_inline_edit_observed",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}

for (const marker of [
  'inline-edit-launch = ["dep:url", "dep:uuid"]',
  'url = { version = "2.5", optional = true }',
  'uuid = { workspace = true, optional = true }',
]) need(pagesAdminCargo, marker, "Pages admin feature contract");
forbid(
  featureBody(pagesAdminCargo, "default", "Pages admin manifest"),
  "inline-edit-launch",
  "Pages admin default feature",
);
for (const feature of ["csr", "hydrate", "ssr"]) {
  forbid(
    featureBody(pagesAdminCargo, feature, "Pages admin manifest"),
    "inline-edit-launch",
    `Pages admin ${feature} profile`,
  );
}

need(
  appAdminCargo,
  'pages-inline-edit-launch = ["rustok-pages-admin/inline-edit-launch"]',
  "admin app launch pass-through",
);
for (const feature of ["default", "csr", "hydrate", "ssr"]) {
  forbid(
    featureBody(appAdminCargo, feature, "admin app manifest"),
    "pages-inline-edit-launch",
    `admin app ${feature} profile`,
  );
}

for (const marker of [
  '#[cfg(feature = "inline-edit-launch")]\nmod inline_edit_launch;',
  '#[cfg(feature = "inline-edit-launch")]\nuse inline_edit_launch::PagesInlineEditLaunch;',
  '#[cfg(feature = "inline-edit-launch")]\n    let inline_edit_launch = view!',
  '<PagesInlineEditLaunch selected_page />',
  '{inline_edit_launch}',
  '#[cfg(not(feature = "inline-edit-launch"))]',
]) need(pagesAdminLib, marker, "Pages admin shell mount");

for (const marker of [
  'const PAGES_AUTHORING_PATH: &str = "/modules/pages-authoring"',
  'option_env!("RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN")',
  'value.eq_ignore_ascii_case("true")',
  "Uuid::parse_str",
  "page_id.is_nil()",
  "transport::fetch_page(token, tenant, page_id).await?",
  'page.status.eq_ignore_ascii_case("published")',
  "translation.locale.as_str()",
  "body.locale.as_str()",
  "const MAX_LOCALE_LENGTH: usize = 64",
  "locale.chars().any(char::is_control)",
  "Serializer::new(String::new())",
  '.append_pair("page_id"',
  '.append_pair("lang"',
  "pages_editor_capability_policy_for_role",
  ".effective\n                        .edit",
  'target="_blank"',
  'rel="noopener noreferrer"',
  'data-pages-inline-edit-launch="same-origin"',
  '"Draft-only. Opens the exact-locale same-origin authoring route',
]) need(launch, marker, "Pages admin launch source");
for (const marker of [
  "RUSTOK_API_URL",
  "RUSTOK_GRAPHQL_URL",
  "window.location",
  "api_base_url",
  "authorization_proof",
  "data-pages-page-id",
  "data-pages-locale",
  "bearer",
  "access_token",
]) forbid(launch, marker, "Pages admin launch source");

for (const marker of [
  "admin-launch-source-ready",
  "Admin-owned inline authoring launch: source-ready",
  "admin asset build integration remains pending",
  "release workflow and admin launch integration remain pending",
]) need(plan, marker, "canonical Pages/Page Builder plan");
for (const marker of [
  "admin-launch-source-ready",
  "Admin-owned inline edit launch: source-ready",
  "admin-owned launch link remains pending",
  "admin asset build integration remains pending",
]) need(localPlan, marker, "Pages implementation plan");
for (const marker of [
  "source-ready / admin-asset-build-and-browser-execution-pending",
  "RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN=true",
  "rustok-admin/pages-inline-edit-launch",
  "noopener noreferrer",
  "Execution evidence remains pending",
]) need(packet, marker, "admin launch packet");

if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-admin-launch] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-inline-edit-admin-launch] PASS source_ready=true exact_draft_identity=true admin_asset_build=pending browser=pending",
);
