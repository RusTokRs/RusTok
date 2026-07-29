#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-catalog-grpc-service-host.mjs",
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-service-host-"));
  write(root, "Cargo.toml", `members = ["crates/*"]`);
  write(
    root,
    "crates/rustok-product-catalog-service/Cargo.toml",
    `name = "rustok-product-catalog-service"
rustok-api.workspace = true
rustok-outbox.workspace = true
rustok-product.workspace = true
rustok-product-transport = { path = "../rustok-product-transport" }
rustok-telemetry.workspace = true
sea-orm.workspace = true
tonic = { workspace = true, features = ["tls-ring"] }`,
  );

  const ownerComposition = options.missingOwnerComposition
    ? ""
    : `CatalogService::new(database, event_bus) OutboxTransport::new(database.clone()) TransactionalEventBus::new(outbox) ProductCatalogGrpcService::new(provider)`;
  const authentication = options.missingAuthentication
    ? ""
    : `ProductCatalogGrpcBearerInterceptor::from_bearer_token( PortActor::service(config.trusted_service_actor.clone()) ProductCatalogReadServiceServer::with_interceptor(service, interceptor) required_secret_env(BEARER_TOKEN_ENV) validate_service_actor(required_env(`;
  const security = options.insecurePublicPlaintext
    ? ""
    : `TLS_CERT_PATH_ENV} and {TLS_KEY_PATH_ENV} must be configured together bind.ip().is_loopback() Product catalog service TLS is required unless explicit loopback plaintext is enabled`;
  const schemaPreflight = options.missingSchemaPreflight
    ? ""
    : `SysEvents entities::{Product, ProductVariant} EntityTrait
REQUIRED_SCHEMA_TABLES: [&str; 3] = ["products", "product_variants", "sys_events"]
async fn verify_required_schema(database: &DatabaseConnection)
Product::find() ProductVariant::find() SysEvents::find()
schema_preflight_error("products") schema_preflight_error("product_variants") schema_preflight_error("sys_events")
run platform migrations before starting the service Product catalog database schema preflight passed`;
  const startupOrder = options.misorderedSchemaPreflight
    ? `verify_required_schema(&database).await?;
let database = connect_database(&config).await?;
let outbox = Arc::new(OutboxTransport::new(database.clone()));
let mut server = Server::builder();`
    : `let database = connect_database(&config).await?;
verify_required_schema(&database).await?;
let outbox = Arc::new(OutboxTransport::new(database.clone()));
let mut server = Server::builder();`;
  const secretLeak = options.leakedSecret ? "bearer_token = %token" : "";
  write(
    root,
    "crates/rustok-product-catalog-service/src/main.rs",
    `
${ownerComposition}
${authentication}
${security}
${schemaPreflight}
${startupOrder}
${secretLeak}
ServerTlsConfig::new().identity(identity) .serve_with_shutdown(config.bind, shutdown_signal())
DATABASE_URL_ENV: &str = "RUSTOK_PRODUCT_CATALOG_DATABASE_URL"
BEARER_TOKEN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN"
TRUSTED_SERVICE_ACTOR_ENV: &str = "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR"
TLS_CERT_PATH_ENV: &str = "RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_CERT_PATH"
TLS_KEY_PATH_ENV: &str = "RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_KEY_PATH"
"RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK"
matches!(parsed.scheme(), "postgres" | "postgresql")
struct RedactedSecret(String) formatter.write_str("[REDACTED]") database_target = %config.database_target options.sqlx_logging(false)
map_err(|_| anyhow!("Product catalog PostgreSQL connection failed"))
service_name: "rustok-product-catalog-service".to_string() rustok_telemetry::init(telemetry_config)? rustok_telemetry::otel::shutdown().await
tokio::signal::ctrl_c() SignalKind::terminate() Product catalog gRPC service shutdown requested
secrets_are_redacted_from_debug_output database_must_be_postgresql_and_debug_target_excludes_credentials required_schema_tables_are_owner_and_outbox_tables plaintext_requires_explicit_loopback_bind tls_requires_certificate_and_key_as_a_pair trusted_service_actor_is_server_configured_and_bounded
`,
  );
  write(
    root,
    "crates/rustok-product-catalog-service/README.md",
    `standalone provider-side deployment unit CatalogService ProductCatalogGrpcService ProductCatalogGrpcBearerInterceptor OutboxTransport read-only does not run migrations at startup ## Schema preflight products product_variants sys_events before tonic starts listening does not silently continue with partial readiness RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_CERT_PATH RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK=true RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR cargo run -p rustok-product-catalog-service does not claim this command was executed boundary_ready`,
  );

  write(
    root,
    "crates/rustok-product/contracts/product-fba-registry.json",
    JSON.stringify({
      status: options.falsePromotion ? "transport_verified" : "boundary_ready",
      evidence: {
        grpc_service_host_verifier:
          "scripts/verify/verify-product-catalog-grpc-service-host.mjs",
      },
      external_transport: {
        provider_host_crate: "rustok-product-catalog-service",
        provider_host_source:
          "crates/rustok-product-catalog-service/src/main.rs",
        provider_host_binary: "rustok-product-catalog-service",
        provider_host_database: "postgresql",
        provider_host_transport_security: "tls_or_explicit_loopback",
        provider_host_authenticator: "ProductCatalogGrpcBearerInterceptor",
        provider_host_trusted_actor_env:
          "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR",
        provider_host_schema_preflight_tables: [
          "products",
          "product_variants",
          "sys_events",
        ],
        provider_host_schema_preflight_order:
          "after_connect_before_owner_and_listener",
        provider_host_schema_preflight_status: options.falseSchemaExecution
          ? "runtime_verified"
          : "source_complete_execution_pending",
        provider_host_status: options.falseExecution
          ? "runtime_verified"
          : "source_complete_execution_pending",
      },
    }),
  );
  write(
    root,
    "crates/rustok-product/docs/implementation-plan.md",
    options.missingPlan
      ? "Product plan"
      : "standalone Product catalog service host is source-complete rustok-product-catalog-service OutboxTransport TLS-by-default schema preflight is source-complete products product_variants sys_events Schema-preflight execution evidence remains open provider-host execution evidence remains open Product remains `boundary_ready` cargo run -p rustok-product-catalog-service verify-product-catalog-grpc-service-host.mjs",
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
    assert.notEqual(result.status, 0, "expected Product service-host mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("Product catalog service-host guard accepts canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("guard rejects missing owner and outbox composition", () => {
  reject({ missingOwnerComposition: true }, /Product owner gRPC host composition/);
});

test("guard rejects missing TLS and loopback fail-closed policy", () => {
  reject({ insecurePublicPlaintext: true }, /fail-closed configuration/);
});

test("guard rejects unauthenticated provider host", () => {
  reject({ missingAuthentication: true }, /owner gRPC host composition|secret and authority/);
});

test("guard rejects missing schema preflight", () => {
  reject({ missingSchemaPreflight: true }, /Product service schema preflight/);
});

test("guard rejects schema preflight after owner composition", () => {
  reject({ misorderedSchemaPreflight: true }, /startup preflight order/);
});

test("guard rejects credential logging", () => {
  reject({ leakedSecret: true }, /ownership and secret boundary/);
});

test("guard rejects premature schema-preflight execution claim", () => {
  reject({ falseSchemaExecution: true }, /provider_host_schema_preflight_status/);
});

test("guard rejects premature host execution claim", () => {
  reject({ falseExecution: true }, /provider_host_status/);
});

test("guard rejects premature Product promotion", () => {
  reject({ falsePromotion: true }, /remain boundary_ready/);
});

test("guard rejects missing implementation-plan handoff", () => {
  reject({ missingPlan: true }, /service-host implementation plan/);
});
