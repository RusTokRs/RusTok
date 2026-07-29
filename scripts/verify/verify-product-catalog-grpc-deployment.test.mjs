#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-catalog-grpc-deployment.mjs",
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-grpc-deployment-"));

  write(
    root,
    "crates/rustok-product-transport/Cargo.toml",
    `thiserror.workspace = true\nurl.workspace = true\nsha2.workspace = true\nsubtle = "2"\ntonic = { workspace = true, features = ["tls-ring", "tls-webpki-roots"] }`,
  );
  write(
    root,
    "crates/rustok-product-transport/src/lib.rs",
    `pub mod auth; pub mod connection; GrpcProductCatalogReadConnectionConfig GrpcProductCatalogReadConnectionError ValidatedGrpcProductCatalogReadConnection ProductCatalogGrpcBearerToken`,
  );
  const connection = options.missingTls
    ? `pub struct GrpcProductCatalogReadConnectionConfig; pub fn validated( Url::parse(value.trim()) !parsed.username().is_empty() parsed.password().is_some() parsed.query().is_some() parsed.fragment().is_some() !matches!(parsed.path(), "" | "/") "http" if allow_insecure_loopback && is_loopback_host(parsed.host()) => false InsecureEndpointForbidden MAX_CONNECT_TIMEOUT_MS timeout_ms == 0 || timeout_ms > MAX_CONNECT_TIMEOUT_MS .connect_timeout(validated.connect_timeout) .tcp_keepalive(Some(Duration::from_secs(30))) pub async fn connect(`
    : `pub struct GrpcProductCatalogReadConnectionConfig; pub fn validated( Url::parse(value.trim()) !parsed.username().is_empty() parsed.password().is_some() parsed.query().is_some() parsed.fragment().is_some() !matches!(parsed.path(), "" | "/") "https" => true "http" if allow_insecure_loopback && is_loopback_host(parsed.host()) => false InsecureEndpointForbidden MAX_CONNECT_TIMEOUT_MS timeout_ms == 0 || timeout_ms > MAX_CONNECT_TIMEOUT_MS ClientTlsConfig::new().with_webpki_roots() tls.domain_name(domain) .connect_timeout(validated.connect_timeout) .tcp_keepalive(Some(Duration::from_secs(30))) pub async fn connect(`;
  write(root, "crates/rustok-product-transport/src/connection.rs", connection);
  write(
    root,
    "crates/rustok-product-transport/README.md",
    `RUSTOK_PRODUCT_CATALOG_PROVIDER=embedded RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK=true does not silently fall back to the embedded provider`,
  );

  write(
    root,
    "apps/server/Cargo.toml",
    `mod-product   = ["dep:rustok-product", "dep:rustok-product-transport"\nrustok-product-transport = { path = "../../crates/rustok-product-transport", optional = true }`,
  );
  write(
    root,
    "apps/server/src/services/mod.rs",
    `pub mod product_catalog_deployment;`,
  );
  const fallback = options.silentFallback
    ? `ProductCatalogReadRuntime::in_process unwrap_or_else(|_|`
    : "";
  const authentication = options.missingAuthentication
    ? ""
    : `const GRPC_BEARER_TOKEN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN"; ProductCatalogGrpcBearerToken::new(remote.bearer_token.expose()) .with_authentication(authentication) remote Product catalog authentication configuration failed grpc_requires_a_bearer_token bearer_token_is_not_silently_ignored_in_embedded_mode bearer_secret_debug_is_redacted`;
  write(
    root,
    "apps/server/src/services/product_catalog_deployment.rs",
    `const PROVIDER_ENV: &str = "RUSTOK_PRODUCT_CATALOG_PROVIDER"; const GRPC_ENDPOINT_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT"; ${authentication} const TLS_DOMAIN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_TLS_DOMAIN"; "RUSTOK_PRODUCT_CATALOG_GRPC_CONNECT_TIMEOUT_MS" "RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK" unwrap_or("embedded") "grpc" => { GrpcProductCatalogReadConnectionConfig::new(remote.endpoint) .with_tls_domain(remote.tls_domain) .with_connect_timeout(Duration::from_millis(remote.connect_timeout_ms)) .allow_insecure_loopback(remote.allow_insecure_loopback) .connect() ProductCatalogReadRuntime::external(Arc::new(provider)) ctx.shared_insert( remote Product catalog provider initialization failed remote Product catalog variables require {PROVIDER_ENV}=grpc is required when {PROVIDER_ENV}=grpc must be either embedded or grpc ${fallback}`,
  );
  const configure = "configure_product_catalog_deployment(&runtime_ctx).await?;";
  const appBootstrap =
    "bootstrap_app_runtime(runtime_ctx.clone(), auth_config.clone(), &rustok_settings).await?;";
  write(
    root,
    "apps/server/src/services/server_bootstrap.rs",
    options.bootstrapAfter
      ? `${appBootstrap} ${configure}`
      : `${configure} ${appBootstrap}`,
  );
  write(
    root,
    "apps/server/src/services/commerce_provider_runtime.rs",
    `.shared_get::<rustok_product::ProductCatalogReadRuntime>() .or_else(|| server.shared_get::<rustok_product::ProductCatalogReadRuntime>()) host.with_shared_value(runtime) SharedAiProductCatalogReadPort(runtime.read_port()) ProductCatalogReadProfile::External`,
  );

  const falsePromotion = options.falsePromotion === true;
  const staleRegistry = options.staleRegistry === true;
  write(
    root,
    "crates/rustok-product/contracts/product-fba-registry.json",
    JSON.stringify({
      status: falsePromotion ? "transport_verified" : "boundary_ready",
      evidence: {
        grpc_deployment_verifier:
          "scripts/verify/verify-product-catalog-grpc-deployment.mjs",
      },
      external_transport: {
        connection_config: "GrpcProductCatalogReadConnectionConfig",
        client_authentication: "ProductCatalogGrpcBearerToken",
        host_deployment: "apps/server/src/services/product_catalog_deployment.rs",
        runtime_factory: "ProductCatalogReadRuntime::external",
        provider_selector_env: "RUSTOK_PRODUCT_CATALOG_PROVIDER",
        endpoint_env: "RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT",
        bearer_token_env: "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN",
        authentication_status: "source_complete_execution_pending",
        status: staleRegistry
          ? "source_complete_execution_pending"
          : falsePromotion
            ? "transport_verified"
            : "runtime_wired_execution_pending",
      },
    }),
  );
  write(
    root,
    "crates/rustok-product/docs/implementation-plan.md",
    options.missingPlan
      ? "Product plan"
      : "The production host now owns explicit Product catalog deployment selection. Invalid remote configuration or connection failure aborts startup. RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN. Adapter and production-wiring source are complete. Product remains `boundary_ready` rather than `transport_verified`; configured remote-profile execution evidence remain open. Wire a fail-closed production external Product runtime profile. verify-product-catalog-grpc-deployment.mjs",
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
    assert.notEqual(result.status, 0, "expected Product deployment mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("Product catalog gRPC deployment guard accepts canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("deployment guard rejects missing TLS policy", () => {
  reject({ missingTls: true }, /validated Product gRPC connection/);
});

test("deployment guard rejects missing authentication", () => {
  reject({ missingAuthentication: true }, /server Product catalog deployment/);
});

test("deployment guard rejects silent embedded fallback", () => {
  reject({ silentFallback: true }, /fail-closed deployment/);
});

test("deployment guard rejects configuration after app bootstrap", () => {
  reject({ bootstrapAfter: true }, /before app runtime composition/);
});

test("deployment guard rejects stale registry status", () => {
  reject({ staleRegistry: true }, /runtime_wired_execution_pending/);
});

test("deployment guard rejects false transport promotion", () => {
  reject({ falsePromotion: true }, /remain boundary_ready|runtime_wired_execution_pending/);
});

test("deployment guard rejects missing plan handoff", () => {
  reject({ missingPlan: true }, /Product implementation plan/);
});
