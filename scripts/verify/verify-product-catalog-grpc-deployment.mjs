#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: required Product catalog deployment file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireAll(source, markers, description) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${description}: missing ${marker}`);
  }
}

function forbidAll(source, markers, description) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${description}: forbidden ${marker}`);
  }
}

const transportCargo = read("crates/rustok-product-transport/Cargo.toml");
const transportLib = read("crates/rustok-product-transport/src/lib.rs");
const connection = read("crates/rustok-product-transport/src/connection.rs");
const readme = read("crates/rustok-product-transport/README.md");
const serverCargo = read("apps/server/Cargo.toml");
const services = read("apps/server/src/services/mod.rs");
const deployment = read("apps/server/src/services/product_catalog_deployment.rs");
const bootstrap = read("apps/server/src/services/server_bootstrap.rs");
const composition = read("apps/server/src/services/commerce_provider_runtime.rs");
const registrySource = read("crates/rustok-product/contracts/product-fba-registry.json");
const plan = read("crates/rustok-product/docs/implementation-plan.md");

requireAll(transportCargo, [
  "thiserror.workspace = true",
  "url.workspace = true",
  'features = ["tls-ring", "tls-webpki-roots"]',
  "sha2.workspace = true",
  'subtle = "2"',
], "Product transport connection dependencies");
requireAll(transportLib, [
  "pub mod auth;",
  "pub mod connection;",
  "GrpcProductCatalogReadConnectionConfig",
  "GrpcProductCatalogReadConnectionError",
  "ValidatedGrpcProductCatalogReadConnection",
  "ProductCatalogGrpcBearerToken",
], "Product transport connection exports");
requireAll(connection, [
  "pub struct GrpcProductCatalogReadConnectionConfig",
  "pub fn validated(",
  "pub async fn connect(",
  "Url::parse(value.trim())",
  "!parsed.username().is_empty()",
  "parsed.password().is_some()",
  "parsed.query().is_some()",
  "parsed.fragment().is_some()",
  '!matches!(parsed.path(), "" | "/")',
  '"https" => true',
  '"http" if allow_insecure_loopback && is_loopback_host(parsed.host()) => false',
  "InsecureEndpointForbidden",
  "MAX_CONNECT_TIMEOUT_MS",
  "timeout_ms == 0 || timeout_ms > MAX_CONNECT_TIMEOUT_MS",
  "ClientTlsConfig::new().with_webpki_roots()",
  "tls.domain_name(domain)",
  ".connect_timeout(validated.connect_timeout)",
  ".tcp_keepalive(Some(Duration::from_secs(30)))",
], "validated Product gRPC connection");
forbidAll(connection, ["std::env::var", "CatalogService", "sea_orm"], "Product transport deployment ownership");

requireAll(serverCargo, [
  'mod-product   = ["dep:rustok-product", "dep:rustok-product-transport"',
  'rustok-product-transport = { path = "../../crates/rustok-product-transport", optional = true }',
], "server Product transport dependency");
requireAll(services, ["pub mod product_catalog_deployment;"], "server service exports");
requireAll(deployment, [
  'const PROVIDER_ENV: &str = "RUSTOK_PRODUCT_CATALOG_PROVIDER";',
  'const GRPC_ENDPOINT_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT";',
  'const GRPC_BEARER_TOKEN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN";',
  'const TLS_DOMAIN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_TLS_DOMAIN";',
  '"RUSTOK_PRODUCT_CATALOG_GRPC_CONNECT_TIMEOUT_MS"',
  '"RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK"',
  'unwrap_or("embedded")',
  '"grpc" => {',
  "ProductCatalogGrpcBearerToken::new(remote.bearer_token.expose())",
  "GrpcProductCatalogReadConnectionConfig::new(remote.endpoint)",
  ".with_tls_domain(remote.tls_domain)",
  ".with_connect_timeout(Duration::from_millis(remote.connect_timeout_ms))",
  ".allow_insecure_loopback(remote.allow_insecure_loopback)",
  ".connect()",
  ".with_authentication(authentication)",
  "ProductCatalogReadRuntime::external(Arc::new(provider))",
  "ctx.shared_insert(",
  "remote Product catalog authentication configuration failed",
  "remote Product catalog provider initialization failed",
  "remote Product catalog variables require {PROVIDER_ENV}=grpc",
  "is required when {PROVIDER_ENV}=grpc",
  "must be either embedded or grpc",
  "grpc_requires_a_bearer_token",
  "bearer_token_is_not_silently_ignored_in_embedded_mode",
  "bearer_secret_debug_is_redacted",
], "server Product catalog deployment");
forbidAll(deployment, [
  "ProductCatalogReadRuntime::in_process",
  "CatalogService::new",
  "unwrap_or_else(|_|",
  "bearer_token = %",
  "bearer_token = ?",
], "server Product catalog fail-closed deployment");

requireAll(bootstrap, [
  "configure_product_catalog_deployment(&runtime_ctx).await?;",
  "bootstrap_app_runtime(runtime_ctx.clone(), auth_config.clone(), &rustok_settings).await?;",
], "server bootstrap Product deployment");
const configureIndex = bootstrap.indexOf(
  "configure_product_catalog_deployment(&runtime_ctx).await?;",
);
const bootstrapIndex = bootstrap.indexOf(
  "bootstrap_app_runtime(runtime_ctx.clone(), auth_config.clone(), &rustok_settings).await?;",
);
if (!(configureIndex >= 0 && bootstrapIndex >= 0 && configureIndex < bootstrapIndex)) {
  failures.push("server bootstrap must configure Product deployment before app runtime composition");
}

requireAll(composition, [
  ".shared_get::<rustok_product::ProductCatalogReadRuntime>()",
  ".or_else(|| server.shared_get::<rustok_product::ProductCatalogReadRuntime>())",
  "host.with_shared_value(runtime)",
  "SharedAiProductCatalogReadPort(runtime.read_port())",
  "ProductCatalogReadProfile::External",
], "host-selected Product runtime composition");

requireAll(readme, [
  "RUSTOK_PRODUCT_CATALOG_PROVIDER=embedded",
  "RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc",
  "RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT",
  "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN",
  "RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK=true",
  "does not silently fall back to the embedded provider",
], "Product transport deployment documentation");

let registry;
try {
  registry = JSON.parse(registrySource);
} catch (error) {
  failures.push(`Product FBA registry is invalid JSON: ${error.message}`);
}
if (registry) {
  if (registry.status !== "boundary_ready") {
    failures.push("Product FBA registry must remain boundary_ready before runtime execution evidence");
  }
  const external = registry.external_transport ?? {};
  if (external.connection_config !== "GrpcProductCatalogReadConnectionConfig") {
    failures.push("Product registry must identify the validated connection config");
  }
  if (external.client_authentication !== "ProductCatalogGrpcBearerToken") {
    failures.push("Product registry must identify authenticated client metadata");
  }
  if (external.host_deployment !== "apps/server/src/services/product_catalog_deployment.rs") {
    failures.push("Product registry must identify the server deployment source");
  }
  if (external.runtime_factory !== "ProductCatalogReadRuntime::external") {
    failures.push("Product registry must identify the external runtime factory");
  }
  if (external.provider_selector_env !== "RUSTOK_PRODUCT_CATALOG_PROVIDER") {
    failures.push("Product registry must identify the provider selector environment variable");
  }
  if (external.endpoint_env !== "RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT") {
    failures.push("Product registry must identify the endpoint environment variable");
  }
  if (external.bearer_token_env !== "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN") {
    failures.push("Product registry must identify the bearer token environment variable");
  }
  if (external.authentication_status !== "source_complete_execution_pending") {
    failures.push("Product registry must retain authentication execution-pending status");
  }
  if (external.status !== "runtime_wired_execution_pending") {
    failures.push("Product registry must retain runtime_wired_execution_pending status");
  }
  if (
    registry.evidence?.grpc_deployment_verifier !==
    "scripts/verify/verify-product-catalog-grpc-deployment.mjs"
  ) {
    failures.push("Product registry must link the gRPC deployment verifier");
  }
}

requireAll(plan, [
  "The production host now owns explicit Product catalog deployment selection",
  "Invalid remote configuration or connection failure aborts startup",
  "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN",
  "Adapter and production-wiring source are complete",
  "Product remains `boundary_ready` rather than",
  "remote-profile execution evidence remain open",
  "Wire a fail-closed production external Product runtime profile",
  "verify-product-catalog-grpc-deployment.mjs",
], "Product implementation plan");

if (failures.length > 0) {
  console.error("Product catalog gRPC deployment verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Product catalog gRPC deployment source verification passed");
