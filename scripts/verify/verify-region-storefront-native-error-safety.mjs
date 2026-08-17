#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL("../../", import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), "utf8");

const source = read(
  "crates/rustok-region/storefront/src/transport/native_server_adapter.rs",
);
const cargo = read("crates/rustok-region/storefront/Cargo.toml");
const evidence = JSON.parse(
  read(
    "crates/rustok-region/contracts/evidence/storefront-native-error-safety-source.json",
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
  ['"dep:tracing"', "SSR tracing feature"],
  ["tracing = { workspace = true, optional = true }", "optional tracing dependency"],
]) requireText(cargo, value, label);

for (const [value, label] of [
  ["const REGION_STOREFRONT_NATIVE_OWNER", "native owner constant"],
  ["const REGION_STOREFRONT_NATIVE_BOUNDARY", "native boundary constant"],
  ["fn record_optional_request_context_error", "optional context diagnostic"],
  ["fn map_tenant_context_error", "tenant context mapper"],
  ["fn map_region_runtime_error", "Region owner mapper"],
  ["owner = REGION_STOREFRONT_NATIVE_OWNER", "owner diagnostics"],
  ['owner_operation = "storefront_regions"', "request operation diagnostics"],
  ['owner_operation = "list_regions"', "owner operation diagnostics"],
  ["tenant_id = %tenant.id", "tenant diagnostics"],
  ["channel_id = ?request_context.channel_id", "channel id diagnostics"],
  ["channel_slug = ?request_context.channel_slug", "channel slug diagnostics"],
  ["locale = %request_context.locale", "locale diagnostics"],
  ['code = "region.storefront_request_context_unavailable"', "request context code"],
  ['code = "region.storefront_tenant_context_unavailable"', "tenant context code"],
  ['code = "region.storefront_owner_runtime_failed"', "owner runtime code"],
  ["boundary = REGION_STOREFRONT_NATIVE_BOUNDARY", "boundary diagnostics"],
  ['ServerFnError::new("Region storefront context is unavailable")', "context envelope"],
  ['ServerFnError::new("Storefront regions are temporarily unavailable")', "owner envelope"],
]) requireText(source, value, label);
forbidText(
  source,
  "request_context.correlation_id",
  "removed RequestContext correlation field",
);

for (const [value, label] of [
  ['endpoint = "region/storefront-data"', "endpoint"],
  ["expect_context::<HostRuntimeContext>()", "host runtime composition"],
  ["RegionService::new(runtime_ctx.db_clone())", "Region service composition"],
  ["extract::<rustok_api::RequestContext>()", "request context extraction"],
  ["Ok(request_context) => Some(request_context)", "optional context success"],
  ["record_optional_request_context_error(error);", "optional context failure logging"],
  ["extract::<rustok_api::TenantContext>()", "tenant extraction"],
  ["resolve_requested_locale(", "locale resolver"],
  ["request_context_locale", "request-context locale fallback"],
  ["tenant_default_locale", "tenant-default locale fallback"],
  [".list_regions(", "Region owner operation"],
  ["Some(requested_locale.as_str())", "requested locale input"],
  ["Some(tenant.default_locale.as_str())", "tenant-default fallback input"],
  ["resolve_storefront_regions(regions, selected_region_id)", "selected-region resolution"],
  ["country_tax_policies: value", "country tax-policy mapping"],
  ["tax_rate: policy.tax_rate.normalize().to_string()", "country tax-rate mapping"],
  ["tax_included: policy.tax_included", "country tax-inclusion mapping"],
  [".map_err(ApiError::from)", "outer error conversion"],
]) requireText(source, value, label);

if (countText(source, 'ServerFnError::new("Region storefront context is unavailable")') !== 1) {
  failures.push("tenant context must expose exactly one static context envelope");
}
if (countText(source, 'ServerFnError::new("Storefront regions are temporarily unavailable")') !== 1) {
  failures.push("Region owner failure must expose exactly one static unavailable envelope");
}
forbidText(
  source,
  ".map_err(ServerFnError::new)?",
  "raw extraction or owner error mapping",
);
forbidText(
  source,
  ".await\n            .ok();",
  "silent optional request-context fallback",
);

if (evidence.status !== "region_storefront_native_error_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  tenant_context_static_public_envelope: true,
  optional_request_context_preserved: true,
  optional_request_context_failure_logged: true,
  owner_runtime_static_public_envelope: true,
  correlation_logging_when_available: true,
  tenant_logging_when_available: true,
  channel_logging_when_available: true,
  locale_logging_when_available: true,
  stable_code_logged: true,
  boundary_logged: true,
  endpoint_changed: false,
  locale_precedence_changed: false,
  selected_region_resolution_changed: false,
  country_tax_policy_mapping_changed: false,
  response_dto_changed: false,
  outer_error_variant_changed: false,
  raw_owner_error_public: false,
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
  console.error("Region storefront native error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Region storefront native failures use static public envelopes with correlation-safe SSR diagnostics; runtime evidence remains open",
);
