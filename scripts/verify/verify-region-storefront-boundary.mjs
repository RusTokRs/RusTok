#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const failures = [];
const files = {
  lib: "crates/rustok-region/storefront/src/lib.rs",
  core: "crates/rustok-region/storefront/src/core.rs",
  ui: "crates/rustok-region/storefront/src/ui/leptos.rs",
  transport: "crates/rustok-region/storefront/src/transport/mod.rs",
  native: "crates/rustok-region/storefront/src/transport/native_server_adapter.rs",
  graphql: "crates/rustok-region/storefront/src/transport/graphql_adapter.rs",
  plan: "crates/rustok-region/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
  package: "package.json",
};
const resolve = (file) => path.join(root, file);
for (const file of Object.values(files)) {
  if (!existsSync(resolve(file))) failures.push(`${file}: expected region storefront boundary file`);
}
const read = (file) => readFileSync(resolve(file), "utf8");
const source = Object.fromEntries(Object.entries(files).map(([key, file]) => [key, read(file)]));
const has = (key, marker, message) => {
  if (!source[key].includes(marker)) failures.push(message);
};
const lacks = (key, marker, message) => {
  if (source[key].includes(marker)) failures.push(message);
};

for (const marker of ["mod core;", "mod transport;", "mod ui;", "pub use ui::RegionView;"]) {
  has("lib", marker, `${files.lib}: missing layer marker ${marker}`);
}
for (const marker of ["leptos::", "leptos_", "#[component]", "Resource<", "web_sys::"]) {
  lacks("core", marker, `${files.core}: core must stay Leptos/runtime free (${marker})`);
}
for (const marker of ["RegionErrorEvidence", "RegionErrorViewModel", "RegionErrorDomEvidence", "selected_region_query_update"]) {
  has("core", marker, `${files.core}: missing core-owned parity helper ${marker}`);
}
has("ui", "transport::fetch_regions", `${files.ui}: UI must call transport facade`);
has("ui", "data-region-error-status", `${files.ui}: missing stable error status evidence`);
has("ui", "data-region-error-locale-key", `${files.ui}: missing stable error locale-key evidence`);
for (const marker of ["crate::api", "native_server_adapter::", "graphql_adapter::", "#[server"]) {
  lacks("ui", marker, `${files.ui}: UI must not call raw adapter (${marker})`);
}
for (const marker of ["mod graphql_adapter;", "mod native_server_adapter;", "RegionFetchFallbackPolicy::NativeThenGraphql", "native_server_adapter::fetch_regions", "graphql_adapter::fetch_regions", "RegionTransportError::fallback_failed"]) {
  has("transport", marker, `${files.transport}: missing parity marker ${marker}`);
}
if (source.transport.indexOf("native_server_adapter::fetch_regions") > source.transport.indexOf("graphql_adapter::fetch_regions")) {
  failures.push(`${files.transport}: fallback order must remain native then GraphQL`);
}
has("native", "fetch_storefront_regions_server", `${files.native}: missing native endpoint call`);
has("graphql", "fetch_storefront_regions_graphql", `${files.graphql}: missing GraphQL endpoint call`);
has("plan", "verify-region-storefront-boundary.mjs", `${files.plan}: missing storefront guardrail evidence`);
has("registry", "verify-region-storefront-boundary.mjs", `${files.registry}: missing storefront guardrail evidence`);
for (const marker of ["verify:region:storefront-boundary", "test:verify:region:storefront-boundary", "npm run test:verify:region:storefront-boundary"]) {
  has("package", marker, `${files.package}: missing script marker ${marker}`);
}

if (failures.length) {
  console.error("region storefront boundary verification failed:");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("region storefront boundary verification passed");
