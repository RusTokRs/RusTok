#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
  "..",
);
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

const evidence = JSON.parse(read(
  "crates/rustok-page-builder/contracts/evidence/page-builder-authenticated-inline-edit-adapter-source.json",
));
const flyCargo = read("crates/fly-leptos/Cargo.toml");
const flyRoot = read("crates/fly-leptos/src/root.rs");
const realDom = read("crates/fly-leptos/src/real_dom_inline.rs");
const storefrontCargo = read("crates/rustok-page-builder-storefront/Cargo.toml");
const storefrontLib = read("crates/rustok-page-builder-storefront/src/lib.rs");
const inline = read("crates/rustok-page-builder-storefront/src/inline_edit.rs");
const pagesStorefrontCargo = read("crates/rustok-pages/storefront/Cargo.toml");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const actualization = read(
  "docs/modules/page-builder-parity-actualization-2026-08-06-inline-edit.md",
);
const packet = read(
  "docs/modules/pages-page-builder-authenticated-inline-edit-adapter-packet-2026-08-06.md",
);

if (evidence.format !== "page_builder_authenticated_inline_edit_adapter_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "page_builder_authenticated_inline_edit_adapter_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "real_dom_adapter_owned_by_fly_leptos",
  "page_builder_storefront_inline_feature_is_optional",
  "anonymous_read_only_renderer_remains_uninstrumented",
  "inline_renderer_forces_component_instrumentation",
  "grant_binds_session_page_revision_hash_expiry_and_proof",
  "authorization_proof_is_redacted_from_debug",
  "authorization_proof_is_not_rendered_into_dom",
  "plain_text_is_bounded_and_normalized",
  "dom_is_a_temporary_focusout_buffer",
  "dom_listener_cleanup_restores_attributes",
  "only_allowlisted_components_receive_contenteditable",
  "runtime_bound_conditional_and_repeated_subtrees_are_excluded",
  "provider_and_composite_components_are_excluded",
  "static_leaf_children_in_unowned_layouts_remain_eligible",
  "unchanged_focusout_does_not_consume_grant",
  "server_authorization_port_precedes_mutation",
  "exact_project_hash_precedes_mutation",
  "monotonic_sequence_precedes_mutation",
  "canonical_fly_patch_is_the_only_document_mutation",
  "successful_commit_returns_full_current_project_data",
  "new_grant_is_required_after_document_hash_changes",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "pages_consumer_grant_issuance_added",
  "pages_consumer_save_transport_added",
  "anonymous_storefront_inline_mount_added",
  "database_schema_changed",
  "graphql_schema_changed",
  "rest_http_api_changed",
  "event_schema_changed",
  "page_builder_publish_behavior_changed",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}

for (const marker of [
  'mod real_dom_inline;',
  'pub use real_dom_inline::*;',
]) need(flyRoot, marker, "fly-leptos exports");
for (const marker of [
  '"Node"',
  '"NodeList"',
  '"HtmlElement"',
]) need(flyCargo, marker, "fly-leptos browser capabilities");
for (const marker of [
  'FLY_REAL_DOM_COMPONENT_ATTRIBUTE: &str = "data-fly-component-id"',
  'FLY_REAL_DOM_INLINE_ATTRIBUTE: &str = "data-fly-inline-editable"',
  "MAX_INLINE_TEXT_BYTES: usize = 64 * 1024",
  "pub struct AuthenticatedInlineEditGrant",
  "expected_project_hash: ProjectHash",
  "authorization_proof: String",
  "expires_at_unix_ms: u64",
  '.field("authorization_proof", &"[REDACTED]")',
  "pub struct AuthenticatedInlineEditRequest",
  'set_attribute("contenteditable", "plaintext-only")',
  'add_event_listener_with_callback(\n            "focusout"',
  'remove_event_listener_with_callback(\n                "focusout"',
  "element.inner_text()",
  "normalize_plain_text",
  "snapshot.restore()",
  "impl Drop for MarkedElement",
  "impl Drop for RealDomInlineEditSubscription",
]) need(realDom, marker, "fly-leptos real-DOM adapter");
forbid(realDom, 'add_event_listener_with_callback("input"', "focusout commit boundary");
forbid(realDom, 'data-inline-proof', "authorization proof DOM boundary");

for (const marker of [
  'inline-edit = ["dep:fly-leptos"]',
  'fly-leptos = { path = "../fly-leptos", optional = true, default-features = false }',
  '"fly-leptos?/wasm-client"',
  '"fly-leptos?/ssr"',
]) need(storefrontCargo, marker, "optional storefront inline feature");
for (const marker of [
  '#[cfg(feature = "inline-edit")]',
  'mod inline_edit;',
  'pub use inline_edit::*;',
  "policy.instrument_components = false",
]) need(storefrontLib, marker, "read-only and inline storefront exports");
forbid(pagesStorefrontCargo, "inline-edit", "anonymous Pages storefront feature graph");

for (const marker of [
  "pub trait InlineEditAuthorizationPort",
  "pub struct AuthenticatedInlineEditSession",
  "self.grant.validate_request(&request, now_unix_ms)?",
  "request.sequence <= self.last_sequence",
  "request.expected_project_hash != current_hash",
  "runtime_owned_component_ids",
  "collect_runtime_owned_subtree",
  "ancestor_blocked",
  '"flyRuntimeBindings"',
  '"flyRuntimeConditions"',
  '"flyRuntimeRepeaters"',
  "component.provider.is_some()",
  "!component.children().is_empty()",
  'content.contains("{{")',
  '== Some(request.value.as_str())',
  "InlineEditError::NoContentChange",
  "authorization.authorize(&request)",
  "EditorCommand::Patch",
  'ComponentPatch::default().set_field("content"',
  "GrapesJsCodec::encode_value(self.editor.document())",
  "policy.instrument_components = true",
  "PageBuilderAuthenticatedInlineStorefront",
  "attach_real_dom_inline_editing",
  'data-rustok-page-builder-inline-storefront="true"',
  "data-inline-session",
  "data-inline-revision",
  "data-inline-project-hash",
  "only_static_leaf_text_components_outside_runtime_subtrees_are_editable",
  "authorized_request_applies_one_canonical_fly_patch",
  "unchanged_focusout_does_not_consume_the_one_commit_grant",
  "stale_replay_dynamic_bound_repeated_and_rejected_authorization_fail_closed",
  '"repeated-child"',
]) need(inline, marker, "Page Builder canonical inline session");
forbid(inline, "data-inline-proof", "Page Builder authorization proof DOM boundary");

const apply = between(
  inline,
  "pub fn apply_authorized(",
  "#[derive(Debug, Clone, PartialEq)]",
  "authenticated inline apply owner",
);
ordered(apply, [
  "self.grant.validate_request(&request, now_unix_ms)?",
  "request.sequence <= self.last_sequence",
  "request.expected_project_hash != current_hash",
  "location.page_index != page_index",
  "runtime_owned.contains(&request.component_id)",
  "== Some(request.value.as_str())",
  "authorization.authorize(&request)",
  "self.editor.apply(EditorCommand::Patch",
  "GrapesJsCodec::encode_value(self.editor.document())",
], "identity eligibility no-op authorization mutation ordering");

for (const marker of [
  "authenticated-inline-adapter-source-ready",
  "Authenticated real-DOM inline adapter: source-ready",
  "Pages consumer grant issuance and document-only save mount remain open",
  "unchanged `focusout`",
]) need(plan, marker, "canonical Pages/Page Builder plan");
for (const marker of [
  "current-source overlay",
  "feature-gated authenticated real-DOM adapter",
  "one canonical Fly `EditorCommand::Patch`",
  "Pages consumer grant issuance and save transport remain open",
  "runtime-owned subtrees",
  "unchanged `focusout`",
]) need(actualization, marker, "Page Builder actualization overlay");
for (const marker of [
  "source-ready / execution-pending",
  "The DOM is never accepted as a document tree",
  "focusout",
  "64 KiB",
  "EditorCommand::Patch",
  "new grant",
  "runtime-owned subtree",
  "unchanged focusout",
  "Execution evidence remains pending",
]) need(packet, marker, "authenticated inline adapter packet");

for (const text of [realDom, inline, packet, actualization]) {
  forbid(text, "Iggy", "authenticated inline adapter slice");
}

if (failures.length > 0) {
  console.error("[verify-page-builder-authenticated-inline-edit-adapter] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-page-builder-authenticated-inline-edit-adapter] PASS source_ready=true execution=pending");
