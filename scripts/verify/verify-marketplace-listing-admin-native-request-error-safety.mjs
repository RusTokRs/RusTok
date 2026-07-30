#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL("../../", import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), "utf8");

const cargo = read("crates/rustok-marketplace-listing/admin/Cargo.toml");
const source = read(
  "crates/rustok-marketplace-listing/admin/src/transport/native_server_adapter.rs",
);
const evidence = JSON.parse(
  read(
    "crates/rustok-marketplace-listing/contracts/evidence/admin-native-request-error-safety-source.json",
  ),
);

const failures = [];
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;

for (const [value, label] of [
  ['"dep:tracing"', "admin SSR tracing feature"],
  ["tracing = { workspace = true, optional = true }", "admin tracing dependency"],
]) requireText(cargo, value, label);

for (const [value, label] of [
  ["const MARKETPLACE_LISTING_ADMIN_NATIVE_OWNER", "native owner constant"],
  ["const MARKETPLACE_LISTING_ADMIN_NATIVE_OPERATION", "native operation constant"],
  ["const MARKETPLACE_LISTING_ADMIN_NATIVE_BOUNDARY", "native boundary constant"],
  ["fn map_runtime_dependency_error(", "runtime dependency mapper"],
  ["fn map_auth_context_error<E: std::fmt::Display>(", "auth context mapper"],
  ["fn map_tenant_context_error<E: std::fmt::Display>(", "tenant context mapper"],
  ["fn map_request_context_error<E: std::fmt::Display>(", "request context mapper"],
  ["fn map_module_availability_error<E: std::fmt::Display>(", "module availability mapper"],
  ["owner = MARKETPLACE_LISTING_ADMIN_NATIVE_OWNER", "owner diagnostics"],
  ["owner_operation = MARKETPLACE_LISTING_ADMIN_NATIVE_OPERATION", "operation diagnostics"],
  ["action = ?action", "action diagnostics"],
  ["correlation_id = %request.correlation_id", "correlation diagnostics"],
  ["tenant_id = %tenant_id", "tenant diagnostics"],
  ["channel_id = ?request.channel_id", "channel id diagnostics"],
  ["channel_slug = ?request.channel_slug", "channel slug diagnostics"],
  ["locale = %request.locale", "locale diagnostics"],
  ["boundary = MARKETPLACE_LISTING_ADMIN_NATIVE_BOUNDARY", "boundary diagnostics"],
  ["error = %error", "internal cause diagnostics"],
]) requireText(source, value, label);

for (const [value, label] of [
  ['endpoint = "marketplace-listing/directory"', "directory endpoint"],
  ['endpoint = "marketplace-listing/detail"', "detail endpoint"],
  ['endpoint = "marketplace-listing/command"', "command endpoint"],
  ["native_request(MarketplaceListingAdminAction::List, None)", "directory request action"],
  ["native_request(MarketplaceListingAdminAction::Read, None)", "detail request action"],
  ["native_request(command_action(&command), Some(idempotency_key))", "command request action"],
  ["action.permission()", "action permission mapping"],
  ["request.user_id != Some(auth.user_id)", "request identity check"],
  ['is_tenant_module_enabled(host.db(), tenant.id, "marketplace_listing")', "module availability check"],
  ["Permission denied: marketplace listing permission required", "permission public message"],
  ["Permission denied: marketplace listing request identity mismatch", "identity public message"],
  ["Marketplace listing module is not enabled for this tenant", "module-disabled public message"],
  [".with_deadline(std::time::Duration::from_secs(5))", "owner port deadline"],
  ['format!("native-marketplace-listing-{}", uuid::Uuid::new_v4())', "owner correlation composition"],
  ["request.locale,", "owner locale composition"],
  ["if let Some(channel) = request.channel_slug", "owner channel composition"],
  ["context.with_idempotency_key(key.to_string())", "owner idempotency composition"],
  ["fn map_port_error(error: rustok_api::PortError)", "PortError mapper"],
  ["PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict", "domain-safe PortError variants"],
  ["PortErrorKind::Unavailable | PortErrorKind::Timeout", "availability PortError variants"],
  ["PortErrorKind::InvariantViolation", "invariant PortError variant"],
]) requireText(source, value, label);

for (const [value, label] of [
  ["marketplace_listing.admin_runtime_unavailable", "runtime stable code"],
  ["marketplace_listing.admin_auth_context_unavailable", "auth stable code"],
  ["marketplace_listing.admin_tenant_context_unavailable", "tenant stable code"],
  ["marketplace_listing.admin_request_context_unavailable", "request stable code"],
  ["marketplace_listing.admin_module_availability_failed", "module-check stable code"],
  ["Marketplace listing service is temporarily unavailable", "runtime public message"],
  ["Marketplace listing request context is unavailable", "context public message"],
]) requireText(source, value, label);

if (countText(source, 'ServerFnError::new("Marketplace listing service is temporarily unavailable")') !== 2) {
  failures.push("runtime dependency and module-check failures must share two stable service envelopes");
}
if (countText(source, 'ServerFnError::new("Marketplace listing request context is unavailable")') !== 3) {
  failures.push("auth, tenant and request extraction must share three stable context envelopes");
}
if (countText(source, ".map_err(map_port_error)") !== 4) {
  failures.push("directory, detail and command owner calls must preserve four PortError mappings");
}
if (countText(source, "NativeMarketplaceListingAdminError") < 6) {
  failures.push("native facade error wrapper must remain present");
}

for (const value of [
  "marketplace listing host runtime is not mounted",
  "marketplace listing owner ports are not composed",
  ".map_err(ServerFnError::new)?",
  'ServerFnError::new("marketplace listing module availability check failed")',
]) forbidText(source, value, "raw marketplace-listing admin request-boundary mapping");

if (evidence.status !== "marketplace_listing_admin_native_request_error_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  host_runtime_static_public_envelope: true,
  owner_runtime_static_public_envelope: true,
  auth_context_static_public_envelope: true,
  tenant_context_static_public_envelope: true,
  request_context_static_public_envelope: true,
  module_availability_failure_static_public_envelope: true,
  permission_messages_changed: false,
  identity_mismatch_message_changed: false,
  module_disabled_message_changed: false,
  port_error_mapper_changed: false,
  port_context_contract_changed: false,
  endpoint_contract_changed: false,
  command_payload_changed: false,
  raw_request_boundary_error_public: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "native_runtime_proven",
  "mounted_parity_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error("Marketplace-listing admin native request error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ marketplace-listing admin native request failures retain SSR diagnostics and static public envelopes; runtime evidence remains open",
);
