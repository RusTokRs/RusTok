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
    failures.push(`${relativePath}: required Product gRPC authentication file is missing`);
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

const cargo = read("crates/rustok-product-transport/Cargo.toml");
const lib = read("crates/rustok-product-transport/src/lib.rs");
const auth = read("crates/rustok-product-transport/src/auth.rs");
const client = read("crates/rustok-product-transport/src/client.rs");
const server = read("crates/rustok-product-transport/src/server.rs");
const deployment = read("apps/server/src/services/product_catalog_deployment.rs");
const readme = read("crates/rustok-product-transport/README.md");
const registrySource = read("crates/rustok-product/contracts/product-fba-registry.json");
const plan = read("crates/rustok-product/docs/implementation-plan.md");

requireAll(cargo, [
  "sha2.workspace = true",
  'subtle = "2"',
  "uuid.workspace = true",
], "Product transport authentication dependencies");
requireAll(lib, [
  "pub mod auth;",
  "ProductCatalogGrpcAuthenticationError",
  "ProductCatalogGrpcBearerToken",
  "ProductCatalogGrpcBearerAuthenticator",
  "ProductCatalogGrpcBearerInterceptor",
], "Product transport authentication exports");
requireAll(auth, [
  'AUTHORIZATION_METADATA: &str = "authorization"',
  'TENANT_ID_METADATA: &str = "x-rustok-tenant-id"',
  "MAX_BEARER_TOKEN_BYTES",
  "Sha256::digest(value)",
  "Sha256::digest(candidate)",
  "ConstantTimeEq",
  ".ct_eq(&candidate_digest)",
  'field("authorization", &"[REDACTED]")',
  "InvalidBearerToken",
  "bearer_token_debug_is_redacted",
], "Product bearer credential contract");
forbidAll(auth, [
  "pub fn authorization_value",
  "pub fn matches_authorization",
  "println!",
  "eprintln!",
], "Product bearer secret exposure");

requireAll(client, [
  "authentication: Option<ProductCatalogGrpcBearerToken>",
  "pub fn with_authentication(",
  "pub fn with_bearer_token(",
  "AUTHORIZATION_METADATA",
  "TENANT_ID_METADATA",
  "Uuid::parse_str(context.tenant_id.as_str())",
  ".insert(AUTHORIZATION_METADATA, authentication.authorization_value())",
  ".insert(TENANT_ID_METADATA, tenant_id)",
  "authenticated_request_carries_bearer_and_tenant_metadata",
  "authenticated_request_rejects_invalid_tenant_metadata",
], "Product authenticated gRPC client");
forbidAll(client, [
  "tracing::info!(authentication",
  "tracing::debug!(authentication",
  "format!(\"Bearer {secret}\")",
], "Product client credential logging");

requireAll(server, [
  "pub struct ProductCatalogGrpcBearerAuthenticator",
  "pub struct ProductCatalogGrpcBearerInterceptor",
  "ProductCatalogGrpcOperation::ALL",
  "self.token.matches_authorization(authorization.as_bytes())",
  "Uuid::parse_str(tenant_id)",
  "actor: self.actor.clone()",
  "allowed_operations: self.allowed_operations.clone()",
  'Status::unauthenticated("Product catalog service authentication failed")',
  "request.extensions_mut().insert(authority)",
  "bearer_interceptor_authenticates_tenant_and_service_actor",
  "bearer_interceptor_rejects_missing_or_wrong_token",
  "bearer_interceptor_rejects_invalid_tenant_metadata",
], "Product authenticated gRPC server");
forbidAll(server, [
  "authorization = %authorization",
  "token = %self.token",
  "secret = %",
], "Product server credential logging");

requireAll(deployment, [
  'GRPC_BEARER_TOKEN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN"',
  "struct ProductCatalogBearerSecret(String)",
  'formatter.write_str("[REDACTED]")',
  "optional_secret_env(GRPC_BEARER_TOKEN_ENV)",
  "ProductCatalogGrpcBearerToken::new(remote.bearer_token.expose())",
  ".with_authentication(authentication)",
  "remote Product catalog authentication configuration failed",
  "bearer_token_is_not_silently_ignored_in_embedded_mode",
  "grpc_requires_a_bearer_token",
  "bearer_secret_debug_is_redacted",
], "Product server authentication deployment");
forbidAll(deployment, [
  "value.trim().to_string() // secret",
  "bearer_token = %",
  "bearer_token = ?",
], "Product deployment credential exposure");

requireAll(readme, [
  "ProductCatalogGrpcBearerToken",
  "ProductCatalogGrpcBearerInterceptor",
  "Authorization: Bearer ...",
  "x-rustok-tenant-id",
  "compares the complete authorization value in constant time",
  "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN",
  "TLS and authentication solve separate problems",
  "must use `ProductCatalogGrpcBearerInterceptor`",
], "Product authentication documentation");

let registry;
try {
  registry = JSON.parse(registrySource);
} catch (error) {
  failures.push(`Product FBA registry is invalid JSON: ${error.message}`);
}
if (registry) {
  if (registry.status !== "boundary_ready") {
    failures.push("Product must remain boundary_ready before authenticated runtime evidence");
  }
  const external = registry.external_transport ?? {};
  if (external.client_authentication !== "ProductCatalogGrpcBearerToken") {
    failures.push("Product registry client authentication identity drift");
  }
  if (external.server_authenticator !== "ProductCatalogGrpcBearerInterceptor") {
    failures.push("Product registry server authenticator identity drift");
  }
  if (external.bearer_token_env !== "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN") {
    failures.push("Product registry bearer token environment drift");
  }
  if (external.authentication_status !== "source_complete_execution_pending") {
    failures.push("Product authentication must remain source_complete_execution_pending");
  }
  for (const metadata of ["authorization", "x-rustok-tenant-id"]) {
    if (!external.authentication_metadata?.includes(metadata)) {
      failures.push(`Product authentication metadata must include ${metadata}`);
    }
  }
  for (const assertion of [
    "constant_time_complete_authorization_compare",
    "tenant_uuid_validated",
    "trusted_service_actor_server_configured",
    "secret_debug_redacted",
    "authentication_failure_secret_not_echoed",
  ]) {
    if (!external.authentication_assertions?.includes(assertion)) {
      failures.push(`Product authentication must assert ${assertion}`);
    }
  }
  if (
    registry.evidence?.grpc_authentication_verifier !==
    "scripts/verify/verify-product-catalog-grpc-authentication.mjs"
  ) {
    failures.push("Product registry must link the gRPC authentication verifier");
  }
}

requireAll(plan, [
  "service-to-service bearer authentication",
  "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN",
  "constant-time",
  "trusted service actor",
  "authentication source",
  "Product remains `boundary_ready`",
  "Implement a standalone Product catalog service host",
  "verify-product-catalog-grpc-authentication.mjs",
], "Product authentication implementation plan");

if (failures.length > 0) {
  console.error("Product catalog gRPC authentication verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Product catalog gRPC authentication source verification passed");
