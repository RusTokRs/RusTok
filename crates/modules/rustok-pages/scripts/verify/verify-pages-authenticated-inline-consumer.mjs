#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const failures = [];
const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const ordered = (text, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = text.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
    previous = index;
  }
};
const between = (text, start, end, label) => {
  const from = text.indexOf(start);
  const to = from < 0 ? -1 : text.indexOf(end, from + start.length);
  if (from < 0 || to < 0) {
    failures.push(`${label}: unable to locate source slice`);
    return "";
  }
  return text.slice(from, to);
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
  "crates/rustok-pages/contracts/evidence/pages-authenticated-inline-consumer-source.json",
));
const grant = read("crates/rustok-pages/src/services/page/inline_edit.rs");
const feature = read("crates/rustok-pages/src/services/page/inline_edit_feature.rs");
const runtime = read("crates/rustok-pages/src/services/page/inline_edit_runtime.rs");
const document = read("crates/rustok-pages/src/services/page/document.rs");
const pageServices = read("crates/rustok-pages/src/services/page/mod.rs");
const services = read("crates/rustok-pages/src/services/mod.rs");
const pagesLib = read("crates/rustok-pages/src/lib.rs");
const storefrontCargo = read("crates/rustok-pages/storefront/Cargo.toml");
const storefrontLib = read("crates/rustok-pages/storefront/src/lib.rs");
const storefront = read("crates/rustok-pages/storefront/src/inline_edit.rs");
const hostCargo = read("apps/storefront/Cargo.toml");
const serverCargo = read("apps/server/Cargo.toml");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/rustok-pages/docs/implementation-plan.md");
const packet = read(
  "docs/modules/pages-page-builder-authenticated-inline-consumer-packet-2026-08-06.md",
);

if (evidence.format !== "pages_authenticated_inline_consumer_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_authenticated_inline_consumer_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "direct_user_principal_required",
  "authenticated_session_bound_separately_from_edit_session",
  "fresh_edit_session_issued_per_grant",
  "pages_update_owner_reused",
  "tenant_inline_capability_defaults_off",
  "published_document_remains_immutable",
  "exact_locale_translation_and_body_required",
  "grapesjs_document_required",
  "stable_fly_page_and_component_ids_required",
  "grant_binds_tenant_actor_auth_session_edit_session_channel_page_locale_revision_hash_and_expiry",
  "grant_version_and_ttl_are_bounded",
  "hmac_sha256_signature_added",
  "fixed_work_signature_comparison_used",
  "bounded_key_rotation_contract_added",
  "host_keyring_has_no_insecure_default",
  "secret_keyring_and_proof_debug_are_redacted",
  "native_server_function_transport_added",
  "proof_is_reverified_immediately_before_mutation",
  "canonical_page_builder_inline_session_is_reused",
  "canonical_fly_patch_remains_the_only_document_mutation",
  "tenant_capability_is_rechecked_before_persistence",
  "existing_save_document_owner_is_the_only_persistence_path",
  "optimistic_body_revision_is_reused",
  "replacement_grant_is_issued_after_committed_revision",
  "safe_server_error_code_and_user_message_are_returned",
  "server_storefront_and_hydrate_profiles_are_opt_in",
  "retained_anonymous_profiles_do_not_enable_inline_edit",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "authenticated_route_mount_added",
  "anonymous_storefront_mount_added",
  "database_schema_changed",
  "graphql_schema_changed",
  "rest_http_api_changed",
  "page_publish_behavior_changed",
  "event_schema_changed",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}

for (const marker of [
  "PAGE_INLINE_EDIT_GRANT_VERSION: u16 = 1",
  "DEFAULT_PAGE_INLINE_EDIT_GRANT_TTL_MS: u64 = 60_000",
  "MAX_PAGE_INLINE_EDIT_GRANT_TTL_MS: u64 = 300_000",
  "MAX_PAGE_INLINE_EDIT_KEYS: usize = 8",
  "pub struct PageInlineEditSecret",
  '.field(&"[REDACTED]")',
  "pub struct PageInlineEditKeyring",
  "pub auth_session_id: Uuid",
  "pub session_id: Uuid",
  "auth_session_id: context.auth_session_id",
  "session_id: context.session_id",
  "hmac_sha256(",
  "fixed_work_sha256_eq(&expected, &signed.signature)",
  "self.auth_session_id.is_nil()",
  "self.session_id.is_nil()",
  "pub async fn load_inline_edit_document(",
  "self.ensure_builder_inline_edit_enabled_for_tenant(tenant_id)",
  "enforce_owned_scope(",
  "Resource::Pages",
  "Action::Update",
  "ensure_document_is_mutable(&page)?",
  "PAGE_BUILDER_DOCUMENT_FORMAT",
]) need(grant, marker, "Pages inline grant owner");
forbid(grant, "default-secret", "Pages inline grant owner");

for (const marker of [
  'FEATURE_BUILDER_INLINE_EDIT_ENABLED: &str = "pages.builder.inline_edit.enabled"',
  '.and_then(|builder| builder.get("inline_edit"))',
  '.and_then(|inline_edit| inline_edit.get("enabled"))',
  ".unwrap_or(false)",
]) need(feature, marker, "tenant inline feature gate");
for (const marker of [
  'PAGES_INLINE_EDIT_HMAC_KEY_ENV: &str = "RUSTOK_PAGES_INLINE_EDIT_HMAC_KEY"',
  "Err(env::VarError::NotPresent) => return Ok(None)",
  "PageInlineEditSecret::new(secret)?",
]) need(runtime, marker, "host keyring composition");
forbid(runtime, "unwrap_or_else", "host keyring composition");

need(document, "pub(super) fn ensure_document_is_mutable", "shared document owner");
for (const marker of [
  "mod inline_edit;",
  "mod inline_edit_feature;",
  "mod inline_edit_runtime;",
  "PageInlineEditKeyring",
  "FEATURE_BUILDER_INLINE_EDIT_ENABLED",
]) need(pageServices, marker, "page service exports");
for (const marker of [
  "PageInlineEditGrantClaims",
  "page_inline_edit_keyring_from_environment",
]) need(services, marker, "service root exports");
for (const marker of [
  "register_runtime_extensions(",
  "page_inline_edit_keyring_from_environment()",
  "extensions.insert(keyring)",
  "Pages inline edit signing runtime registered",
]) need(pagesLib, marker, "Pages module runtime registration");

for (const marker of [
  "inline-edit = [",
  '"dep:fly"',
  '"dep:uuid"',
  '"rustok-page-builder-storefront/inline-edit"',
]) need(storefrontCargo, marker, "Pages storefront inline feature");
for (const baseFeature of ["default", "hydrate", "ssr"]) {
  forbid(
    featureBody(storefrontCargo, baseFeature, "Pages storefront manifest"),
    "inline-edit",
    `Pages storefront ${baseFeature} profile`,
  );
}
for (const marker of [
  '#[cfg(feature = "inline-edit")]',
  "mod inline_edit;",
  "PagesAuthenticatedInlineEditSurface",
]) need(storefrontLib, marker, "Pages storefront inline exports");

for (const marker of [
  "pub struct PagesInlineEditBootstrap",
  '.field("authorization_proof", &"[REDACTED]")',
  'endpoint = "pages/inline-edit/bootstrap"',
  'endpoint = "pages/inline-edit/commit"',
  "principal.kind.is_direct_user()",
  "auth.session_id.is_nil()",
  "PageInlineEditKeyring",
  "ensure_builder_inline_edit_enabled_for_tenant",
  "load_inline_edit_document(",
  "claims.auth_session_id != context.auth.session_id",
  "session_id: uuid::Uuid::new_v4()",
  "document.project.visit_components",
  "stable component ids before hashing",
  "authorization_claims = context",
  ".verify(&proof, authorization_time_unix_ms)",
  "AuthenticatedInlineEditSession::new(",
  ".apply_authorized(",
  ".save_document(",
  "expected_revision: claims.revision_id.clone()",
  "issue_bootstrap_with_identity(",
  "fn pages_server_error(",
  "user_message",
]) need(storefront, marker, "Pages inline consumer");
for (const forbidden of [
  "page_body::ActiveModel",
  "page_body::Entity::update",
  "data-inline-proof",
  "RUSTOK_PAGES_INLINE_EDIT_HMAC_KEY=",
]) forbid(storefront, forbidden, "Pages inline consumer");

const commit = between(
  storefront,
  "async fn pages_inline_edit_commit(",
  "#[component]\npub fn PagesAuthenticatedInlineEditSurface",
  "Pages inline commit owner",
);
ordered(commit, [
  "InlineEditServerContext::extract().await?",
  ".verify(request.authorization_proof(), received_at_unix_ms)",
  "ensure_claims_match_request(&context, &claims, &request)",
  ".load_inline_edit_document(",
  "document.revision_id != claims.revision_id",
  "decode_canonical_document(&document.project_data)",
  ".verify(&proof, authorization_time_unix_ms)",
  "AuthenticatedInlineEditSession::new(",
  ".apply_authorized(",
  ".ensure_builder_inline_edit_enabled_for_tenant(context.tenant_id)",
  ".save_document(",
  "expected_revision: claims.revision_id.clone()",
  "issue_bootstrap_with_identity(",
], "auth proof document patch capability save replacement ordering");

for (const marker of [
  "pages-inline-edit = [",
  '"rustok-pages-storefront/inline-edit"',
  "pages-inline-edit-hydrate = [",
  '"rustok-pages-storefront/hydrate"',
]) need(hostCargo, marker, "storefront host opt-in profiles");
for (const baseFeature of ["csr", "hydrate", "ssr"]) {
  forbid(
    featureBody(hostCargo, baseFeature, "storefront host manifest"),
    "pages-inline-edit",
    `storefront host ${baseFeature} profile`,
  );
}
need(
  serverCargo,
  'pages-inline-edit = ["embed-storefront", "mod-pages", "rustok-storefront/pages-inline-edit"]',
  "self-contained server opt-in profile",
);
forbid(
  featureBody(serverCargo, "default", "server manifest"),
  "pages-inline-edit",
  "server default profile",
);

for (const marker of [
  "authenticated-inline-consumer-source-ready",
  "Pages authenticated inline consumer: source-ready",
  "authenticated route mount remains open",
]) need(plan, marker, "canonical plan");
for (const marker of [
  "authenticated inline grants/save transport",
  "document-only persistence owner",
  "authenticated route mount remains open",
]) need(localPlan, marker, "Pages local plan");
for (const marker of [
  "source-ready / execution-pending",
  "direct authenticated user session",
  "fresh edit-session UUID",
  "existing `PageService::save_document` owner",
  "Route mount: open",
  "Execution evidence remains pending",
]) need(packet, marker, "authenticated inline consumer packet");

for (const text of [grant, runtime, storefront, packet]) {
  forbid(text, "Iggy", "authenticated inline consumer slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-authenticated-inline-consumer] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-authenticated-inline-consumer] PASS source_ready=true execution=pending route_mount=open",
);
