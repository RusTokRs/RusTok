#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL("../../", import.meta.url));
const read = (file) => readFileSync(path.join(root, file), "utf8");
const base = "crates/modules/rustok-product/";
const list = read(`${base}storefront/src/transport/catalog_list_native.rs`);
const facets = read(`${base}storefront/src/transport/catalog_facets_native.rs`);
const native = read(`${base}storefront/src/transport/native_server_adapter.rs`);
const ui = read(`${base}storefront/src/ui/leptos.rs`);
const cargo = read(`${base}storefront/Cargo.toml`);
const evidence = JSON.parse(
  read(
    `${base}contracts/evidence/storefront-catalog-native-error-safety-source.json`,
  ),
);
const failures = [];
const compact = (text) => text.replace(/\s/g, "").replace(/,\)/g, ")");
const requireText = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbidText = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
function body(source, name) {
  const match = new RegExp(`fn\\s+${name}\\s*\\(`).exec(source);
  if (!match) {
    failures.push(`missing native function ${name}`);
    return "";
  }
  const start = source.indexOf("{", match.index);
  let depth = 0;
  for (let index = start; index < source.length; index++) {
    if (source[index] === "{") depth++;
    if (source[index] === "}" && --depth === 0)
      return source.slice(start, index + 1);
  }
  failures.push(`unterminated native function ${name}`);
  return "";
}

requireText(cargo, "tracing.workspace = true", "all-profile diagnostics");
requireText(cargo, '"dep:tokio"', "SSR-only request budget");
for (const [source, name, operation, port] of [
  [
    list,
    "storefront_catalog_list_native",
    "PRODUCT_STOREFRONT_CATALOG_OPERATION",
    "list_filtered_published_products",
  ],
  [
    facets,
    "storefront_catalog_facets_native",
    "PRODUCT_STOREFRONT_FACETS_OPERATION",
    "load_storefront_catalog_facets",
  ],
  [
    native,
    "storefront_product_detail_native",
    '"storefront_product_detail"',
    "read_storefront_product_projection",
  ],
  [
    native,
    "storefront_catalog_search_options_native",
    '"storefront_catalog_search_options"',
    "schema_read_port",
  ],
  [
    native,
    "storefront_buyer_quote_native",
    '"storefront_buyer_quote"',
    "resolve_product_prices",
  ],
]) {
  const block = body(source, name);
  requireText(
    compact(block),
    compact(`product_buyer_scope(${operation}).await?`),
    `${name}: required context propagation`,
  );
  requireText(block, port, `${name}: owner capability`);
  requireText(
    block,
    "product_read_call(",
    `${name}: enforced shared read budget`,
  );
  requireText(
    source,
    "client = leptos_auth::AuthorizedBrowserClient",
    `${name}: authenticated native client`,
  );
  if (block.indexOf("product_buyer_scope(") > block.indexOf(port))
    failures.push(`${name}: admission must precede owner IO`);
}

const scope = body(native, "product_buyer_scope");
for (const marker of [
  "use_context::<rustok_api::HostRuntimeContext>()",
  "ok_or_else(map_product_internal_error)",
  "require_request_context(",
  "extract::<rustok_api::TenantContext>()",
  "extract::<rustok_api::OptionalAuthContext>()",
  "admit_storefront_buyer_request(",
  "shared_get::<rustok_outbox::TransactionalEventBus>()",
  "tokio::time::timeout_at(",
])
  requireText(scope, marker, "shared trusted context admission");
for (const marker of [".ok();", ".unwrap_or_default()", "Err(_) => None"])
  forbidText(scope, marker, "rejected context must not become optional");
requireText(
  compact(native),
  "tokio::time::timeout_at(scope.deadline,call)",
  "all reads share the remaining endpoint budget",
);
requireText(
  native,
  "saturating_duration_since(tokio::time::Instant::now())",
  "remaining owner-port budget",
);
requireText(native, ".schema_read_port()", "selected schema capability");
requireText(native, "scope.host", "host-selected catalog runtime");
requireText(native, "ProductCatalogReadRuntime", "selected runtime contract");

const required = body(native, "require_request_context");
for (const marker of [
  "ProductPublicError::internal()",
  "request_context_rejected = true",
  "correlation_id = %public.correlation_id",
  "ServerFnError::new(public.to_string())",
])
  requireText(required, marker, "required context public mapping");
const internal = body(native, "map_product_internal_error");
for (const marker of [
  "tracing::error!(",
  "correlation_id = %public.correlation_id",
  "code = public.code",
  "ServerFnError::new(public.to_string())",
])
  requireText(internal, marker, "correlated internal public envelope");
for (const [source, label] of [
  [required, "context mapper"],
  [internal, "internal mapper"],
  [body(native, "map_product_port_error"), "port mapper"],
]) {
  for (const marker of [
    "error = ?error",
    "error = %error",
    "error.to_string()",
    "channel_slug = ?",
    "locale = %",
    "token =",
  ])
    forbidText(source, marker, `${label}: raw diagnostic payload`);
}
for (const source of [list, facets, native]) {
  for (const marker of [
    "channel_slug = ?",
    "channel_slug = %",
    "locale = ?",
    "locale = %",
  ])
    forbidText(source, marker, "native adapter: raw diagnostic payload");
  for (const marker of [
    "CatalogService::new(",
    "ProductCatalogSchemaService::new(",
    "expect_context::<HostRuntimeContext>()",
    ".map_err(ServerFnError::new)",
  ])
    forbidText(
      source,
      marker,
      "native adapter must use selected, public-safe owner boundaries",
    );
}
for (const marker of [
  "StorefrontProductListQuery::try_from_transport_with_attribute_filters(",
  ".with_pagination(1, 12)",
  ".with_currency_code(currency_code)",
  "map_product_public_error(",
])
  requireText(list, marker, "catalog input/query contract");
requireText(
  list,
  "native_server_adapter::fetch_product_detail(",
  "detail transport composition",
);
requireText(
  facets,
  'map_product_service_error(error, "storefront_catalog_facets_input")',
  "facet input mapper",
);
for (const marker of [
  ".unwrap_or_else(|_| ProductCatalogSearchOptions::default())",
  ".await\n            .unwrap_or_default()",
])
  forbidText(
    ui,
    marker,
    "failed filter capability must not masquerade as empty options",
  );
requireText(
  compact(ui),
  "Ok::<_,transport::ProductTransportError>((options,facets))",
  "fallible support resource",
);

if (
  evidence.status !==
  "product_storefront_catalog_native_error_safety_source_unvalidated"
)
  failures.push("native source status must remain unvalidated");
for (const key of [
  "request_context_required",
  "request_context_rejection_fail_closed",
  "request_context_failure_logged",
  "selected_runtime_preserved",
  "shared_read_budget_enforced",
])
  if (evidence.source_contract?.[key] !== true)
    failures.push(`evidence source_contract.${key} must be true`);
for (const key of [
  "tests_run",
  "cargo_run",
  "workflow_checks_run",
  "ci_run",
  "native_runtime_proven",
  "mounted_parity_proven",
])
  if (evidence.validation?.[key] !== false)
    failures.push(`evidence validation.${key} must remain false`);

if (failures.length) {
  console.error(
    "Product storefront catalog native error-safety verification failed:",
  );
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "✔ Product native reads share trusted admission, selected capabilities and a bounded budget; runtime evidence remains open",
);
