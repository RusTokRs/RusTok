#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-catalog-grpc-authentication.mjs",
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-grpc-auth-"));
  write(
    root,
    "crates/rustok-product-transport/Cargo.toml",
    `sha2.workspace = true\nsubtle = "2"\nuuid.workspace = true`,
  );
  write(
    root,
    "crates/rustok-product-transport/src/lib.rs",
    `pub mod auth; ProductCatalogGrpcAuthenticationError ProductCatalogGrpcBearerToken ProductCatalogGrpcBearerAuthenticator ProductCatalogGrpcBearerInterceptor`,
  );
  const compare = options.missingConstantTime
    ? ""
    : "Sha256::digest(value) authorization_digest(candidate) .ct_eq(&candidate_digest)";
  const redaction = options.leakedCredential
    ? `field("authorization", &self.authorization)`
    : `field("authorization", &"[REDACTED]")`;
  write(
    root,
    "crates/rustok-product-transport/src/auth.rs",
    `AUTHORIZATION_METADATA: &str = "authorization" TENANT_ID_METADATA: &str = "x-rustok-tenant-id" MAX_BEARER_TOKEN_BYTES ConstantTimeEq ${compare} ${redaction} InvalidBearerToken bearer_token_debug_is_redacted`,
  );
  const tenantInsert = options.missingTenantMetadata
    ? ""
    : `.insert(TENANT_ID_METADATA, tenant_id)`;
  write(
    root,
    "crates/rustok-product-transport/src/client.rs",
    `authentication: Option<ProductCatalogGrpcBearerToken> pub fn with_authentication( pub fn with_bearer_token( AUTHORIZATION_METADATA TENANT_ID_METADATA Uuid::parse_str(context.tenant_id.as_str()) .insert(AUTHORIZATION_METADATA, authentication.authorization_value()) ${tenantInsert} authenticated_request_carries_bearer_and_tenant_metadata authenticated_request_rejects_invalid_tenant_metadata`,
  );
  write(
    root,
    "crates/rustok-product-transport/src/server.rs",
    `pub struct ProductCatalogGrpcBearerAuthenticator pub struct ProductCatalogGrpcBearerInterceptor ProductCatalogGrpcOperation::ALL self.token.matches_authorization(authorization.as_bytes()) Uuid::parse_str(tenant_id) actor: self.actor.clone() allowed_operations: self.allowed_operations.clone() Status::unauthenticated("Product catalog service authentication failed") request.extensions_mut().insert(authority) bearer_interceptor_authenticates_tenant_and_service_actor bearer_interceptor_rejects_missing_or_wrong_token bearer_interceptor_rejects_invalid_tenant_metadata`,
  );
  const tokenRequirement = options.missingDeploymentToken
    ? ""
    : `GRPC_BEARER_TOKEN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN" optional_secret_env(GRPC_BEARER_TOKEN_ENV) ProductCatalogGrpcBearerToken::new(remote.bearer_token.expose()) .with_authentication(authentication) grpc_requires_a_bearer_token bearer_token_is_not_silently_ignored_in_embedded_mode`;
  write(
    root,
    "apps/server/src/services/product_catalog_deployment.rs",
    `struct ProductCatalogBearerSecret(String) formatter.write_str("[REDACTED]") ${tokenRequirement} remote Product catalog authentication configuration failed bearer_secret_debug_is_redacted`,
  );
  write(
    root,
    "crates/rustok-product-transport/README.md",
    `ProductCatalogGrpcBearerToken ProductCatalogGrpcBearerInterceptor Authorization: Bearer ... x-rustok-tenant-id compares the complete authorization value in constant time RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN TLS and authentication solve separate problems must use \`ProductCatalogGrpcBearerInterceptor\``,
  );
  write(
    root,
    "crates/rustok-product/contracts/product-fba-registry.json",
    JSON.stringify({
      status: options.falsePromotion ? "transport_verified" : "boundary_ready",
      evidence: {
        grpc_authentication_verifier:
          "scripts/verify/verify-product-catalog-grpc-authentication.mjs",
      },
      external_transport: {
        client_authentication: "ProductCatalogGrpcBearerToken",
        server_authenticator: "ProductCatalogGrpcBearerInterceptor",
        bearer_token_env: "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN",
        authentication_status: options.falseExecution
          ? "runtime_verified"
          : "source_complete_execution_pending",
        authentication_metadata: ["authorization", "x-rustok-tenant-id"],
        authentication_assertions: [
          "constant_time_complete_authorization_compare",
          "tenant_uuid_validated",
          "trusted_service_actor_server_configured",
          "secret_debug_redacted",
          "authentication_failure_secret_not_echoed",
        ],
      },
    }),
  );
  write(
    root,
    "crates/rustok-product/docs/implementation-plan.md",
    options.missingPlan
      ? "Product plan"
      : "service-to-service bearer authentication RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN constant-time trusted service actor authentication source Product remains `boundary_ready` standalone Product catalog service host is source-complete verify-product-catalog-grpc-authentication.mjs",
  );
  return root;
}

function run(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function reject(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0, "expected authentication mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("authentication guard accepts canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("guard rejects non-constant-time token comparison", () => {
  reject({ missingConstantTime: true }, /Product bearer credential contract/);
});

test("guard rejects credential exposure in Debug", () => {
  reject({ leakedCredential: true }, /Product bearer credential contract/);
});

test("guard rejects missing tenant metadata", () => {
  reject({ missingTenantMetadata: true }, /Product authenticated gRPC client/);
});

test("guard rejects deployment without required bearer token", () => {
  reject({ missingDeploymentToken: true }, /Product server authentication deployment/);
});

test("guard rejects premature authentication execution claim", () => {
  reject({ falseExecution: true }, /source_complete_execution_pending/);
});

test("guard rejects premature Product promotion", () => {
  reject({ falsePromotion: true }, /remain boundary_ready/);
});

test("guard rejects missing implementation-plan handoff", () => {
  reject({ missingPlan: true }, /Product authentication implementation plan/);
});
