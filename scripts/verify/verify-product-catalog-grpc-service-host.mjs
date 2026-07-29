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
    failures.push(`${relativePath}: required Product catalog service host file is missing`);
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

const rootCargo = read("Cargo.toml");
const serviceRoot = "crates/rustok-product-catalog-service";
const cargo = read(`${serviceRoot}/Cargo.toml`);
const source = read(`${serviceRoot}/src/main.rs`);
const readme = read(`${serviceRoot}/README.md`);
const registrySource = read("crates/rustok-product/contracts/product-fba-registry.json");
const plan = read("crates/rustok-product/docs/implementation-plan.md");

requireAll(rootCargo, ['"crates/*"'], "workspace service discovery");
requireAll(cargo, [
  'name = "rustok-product-catalog-service"',
  "rustok-api.workspace = true",
  "rustok-outbox.workspace = true",
  "rustok-product.workspace = true",
  'rustok-product-transport = { path = "../rustok-product-transport" }',
  "rustok-telemetry.workspace = true",
  "sea-orm.workspace = true",
  'tonic = { workspace = true, features = ["tls-ring"] }',
], "Product catalog service manifest");
forbidAll(cargo, [
  "axum",
  "async-graphql",
  "sea-orm-migration",
  "rustok-migrations",
  "rustok-commerce",
], "Product catalog service dependency boundary");

requireAll(source, [
  "CatalogService::new(database, event_bus)",
  "OutboxTransport::new(database.clone())",
  "TransactionalEventBus::new(outbox)",
  "ProductCatalogGrpcService::new(provider)",
  "ProductCatalogGrpcBearerInterceptor::from_bearer_token(",
  "PortActor::service(config.trusted_service_actor.clone())",
  "ProductCatalogReadServiceServer::with_interceptor(service, interceptor)",
  "Server::builder()",
  "ServerTlsConfig::new().identity(identity)",
  ".serve_with_shutdown(config.bind, shutdown_signal())",
], "Product owner gRPC host composition");
requireAll(source, [
  'DATABASE_URL_ENV: &str = "RUSTOK_PRODUCT_CATALOG_DATABASE_URL"',
  'BEARER_TOKEN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN"',
  'TRUSTED_SERVICE_ACTOR_ENV: &str = "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR"',
  'TLS_CERT_PATH_ENV: &str = "RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_CERT_PATH"',
  'TLS_KEY_PATH_ENV: &str = "RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_KEY_PATH"',
  '"RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK"',
  'matches!(parsed.scheme(), "postgres" | "postgresql")',
  "bind.ip().is_loopback()",
  "TLS_CERT_PATH_ENV} and {TLS_KEY_PATH_ENV} must be configured together",
  "Product catalog service TLS is required unless explicit loopback plaintext is enabled",
], "Product service fail-closed configuration");
requireAll(source, [
  "struct RedactedSecret(String)",
  'formatter.write_str("[REDACTED]")',
  "database_target = %config.database_target",
  "options.sqlx_logging(false)",
  'map_err(|_| anyhow!("Product catalog PostgreSQL connection failed"))',
  "required_secret_env(BEARER_TOKEN_ENV)",
  "validate_service_actor(required_env(",
], "Product service secret and authority handling");
requireAll(source, [
  'service_name: "rustok-product-catalog-service".to_string()',
  "rustok_telemetry::init(telemetry_config)?",
  "rustok_telemetry::otel::shutdown().await",
  "tokio::signal::ctrl_c()",
  "SignalKind::terminate()",
  "Product catalog gRPC service shutdown requested",
], "Product service telemetry and shutdown");
requireAll(source, [
  "secrets_are_redacted_from_debug_output",
  "database_must_be_postgresql_and_debug_target_excludes_credentials",
  "plaintext_requires_explicit_loopback_bind",
  "tls_requires_certificate_and_key_as_a_pair",
  "trusted_service_actor_is_server_configured_and_bounded",
], "Product service source tests");
forbidAll(source, [
  "impl ProductCatalogReadPort",
  "ProductCatalogReadRuntime::in_process",
  "MigrationTrait",
  "Migrator::up",
  "axum::",
  "async_graphql",
  "NoopEvent",
  "InMemoryEvent",
  "println!",
  "database_url = %",
  "bearer_token = %",
  "bearer_token = ?",
], "Product service ownership and secret boundary");

requireAll(readme, [
  "standalone provider-side deployment unit",
  "CatalogService",
  "ProductCatalogGrpcService",
  "ProductCatalogGrpcBearerInterceptor",
  "OutboxTransport",
  "read-only",
  "does not run migrations at startup",
  "RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_CERT_PATH",
  "RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK=true",
  "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR",
  "cargo run -p rustok-product-catalog-service",
  "does not claim this command was executed",
  "boundary_ready",
], "Product service deployment documentation");

let registry;
try {
  registry = JSON.parse(registrySource);
} catch (error) {
  failures.push(`Product FBA registry is invalid JSON: ${error.message}`);
}
if (registry) {
  if (registry.status !== "boundary_ready") {
    failures.push("Product must remain boundary_ready before service-host execution evidence");
  }
  const external = registry.external_transport ?? {};
  const expected = {
    provider_host_crate: "rustok-product-catalog-service",
    provider_host_source: "crates/rustok-product-catalog-service/src/main.rs",
    provider_host_binary: "rustok-product-catalog-service",
    provider_host_database: "postgresql",
    provider_host_transport_security: "tls_or_explicit_loopback",
    provider_host_authenticator: "ProductCatalogGrpcBearerInterceptor",
    provider_host_trusted_actor_env: "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR",
    provider_host_status: "source_complete_execution_pending",
  };
  for (const [key, value] of Object.entries(expected)) {
    if (external[key] !== value) {
      failures.push(`Product registry ${key} must be ${value}`);
    }
  }
  if (
    registry.evidence?.grpc_service_host_verifier !==
    "scripts/verify/verify-product-catalog-grpc-service-host.mjs"
  ) {
    failures.push("Product registry must link the gRPC service-host verifier");
  }
}

requireAll(plan, [
  "standalone Product catalog service host is source-complete",
  "rustok-product-catalog-service",
  "OutboxTransport",
  "TLS-by-default",
  "provider-host execution evidence remains open",
  "Product remains `boundary_ready`",
  "cargo run -p rustok-product-catalog-service",
  "verify-product-catalog-grpc-service-host.mjs",
], "Product service-host implementation plan");

if (failures.length > 0) {
  console.error("Product catalog gRPC service-host verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Product catalog gRPC service-host source verification passed");
