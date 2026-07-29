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
    failures.push(`${relativePath}: required Product catalog gRPC transport file is missing`);
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

const crateRoot = "crates/rustok-product-transport";
const cargo = read(`${crateRoot}/Cargo.toml`);
const build = read(`${crateRoot}/build.rs`);
const proto = read(`${crateRoot}/proto/rustok/product/product_catalog.proto`);
const lib = read(`${crateRoot}/src/lib.rs`);
const client = read(`${crateRoot}/src/client.rs`);
const server = read(`${crateRoot}/src/server.rs`);
const conformance = read(`${crateRoot}/tests/port_conformance.rs`);
const readme = read(`${crateRoot}/README.md`);
const registrySource = read("crates/rustok-product/contracts/product-fba-registry.json");
const plan = read("crates/rustok-product/docs/implementation-plan.md");

requireAll(cargo, [
  'name = "rustok-product-transport"',
  "rustok-api.workspace = true",
  "rustok-product.workspace = true",
  "thiserror.workspace = true",
  "tonic = { workspace = true",
  "tonic-prost.workspace = true",
  "url.workspace = true",
  "protoc-bin-vendored.workspace = true",
  "tonic-prost-build.workspace = true",
  'tokio-stream = { version = "0.1", features = ["net"] }',
], "transport Cargo manifest");
forbidAll(cargo, ["sea-orm", "rustok-outbox", "rustok-pricing"], "transport Cargo ownership");

requireAll(build, [
  "protoc_bin_vendored::protoc_bin_path()",
  'proto/rustok/product/product_catalog.proto',
  "tonic_prost_build::configure()",
], "transport build script");
requireAll(proto, [
  "package rustok.product;",
  "service ProductCatalogReadService",
  "rpc ReadProductProjection(JsonRequest) returns (JsonResponse);",
  "rpc ReadVariantProductProjection(JsonRequest) returns (JsonResponse);",
  "rpc ListPublishedProducts(JsonRequest) returns (JsonResponse);",
  "bytes context_json = 1;",
  "bytes input_json = 2;",
  "bytes output_json = 1;",
], "Product catalog protobuf");
requireAll(lib, [
  "pub mod client;",
  "pub mod connection;",
  "pub mod server;",
  'tonic::include_proto!("rustok.product")',
  "GrpcProductCatalogReadProvider",
  "GrpcProductCatalogReadConnectionConfig",
  "ProductCatalogGrpcService",
  "TrustedProductCatalogAuthority",
  "ProductCatalogGrpcOperation",
], "transport public exports");

requireAll(client, [
  "impl ProductCatalogReadPort for GrpcProductCatalogReadProvider",
  "async fn read_product_projection(",
  "async fn read_variant_product_projection(",
  "async fn list_published_products(",
  "context_json: encode(&context)?",
  "input_json: encode(&request)?",
  "request.set_timeout(Duration::from_millis(deadline_ms))",
  "serde_json::from_slice::<PortError>(status.details())",
  "Code::DeadlineExceeded => PortErrorKind::Timeout",
  "Code::Unavailable | Code::ResourceExhausted => PortErrorKind::Unavailable",
], "gRPC client adapter");
forbidAll(client, ["CatalogService", "sea_orm", "crate::entities"], "gRPC client ownership");

requireAll(server, [
  "impl<P> ProductCatalogReadService for ProductCatalogGrpcService<P>",
  "P: ProductCatalogReadPort + 'static",
  "TrustedProductCatalogAuthority",
  "allowed_operations: HashSet<ProductCatalogGrpcOperation>",
  "ReadProductProjection",
  "ReadVariantProductProjection",
  "ListPublishedProducts",
  "trusted_context(",
  "claimed.tenant_id != authority.tenant_id",
  "claimed.actor = authority.actor.clone()",
  "claimed.claims.clone_from(&authority.claims)",
  "claimed.roles.clone_from(&authority.roles)",
  "Status::with_details(code, error.message, Bytes::from(details))",
  'Status::unauthenticated("trusted Product catalog authority is missing")',
  "assert_eq!(trusted.actor, PortActor::service(\"trusted-product-service\"))",
], "gRPC server adapter");
forbidAll(server, ["CatalogService", "sea_orm", "crate::entities"], "gRPC server ownership");

requireAll(conformance, [
  "impl ProductCatalogReadPort for MockProductCatalogReadPort",
  "ProductCatalogReadServiceServer::with_interceptor",
  "GrpcProductCatalogReadProvider::connect",
  "ProductCatalogGrpcOperation::ReadProductProjection",
  "ProductCatalogGrpcOperation::ReadVariantProductProjection",
  "ProductCatalogGrpcOperation::ListPublishedProducts",
  "read_product_projection(",
  "read_variant_product_projection(",
  "list_published_products(",
  "product.product_not_found",
  "port.deadline_required",
  'PortActor::service("trusted-product-catalog-conformance")',
  "serve_with_incoming_shutdown",
], "loopback conformance harness");
requireAll(readme, [
  "Typed tonic gRPC framing",
  "does not own Product DTOs",
  "TrustedProductCatalogAuthority",
  "cargo test -p rustok-product-transport --test port_conformance",
  "does not claim this command was executed",
  "boundary_ready",
], "transport README");

let registry;
try {
  registry = JSON.parse(registrySource);
} catch (error) {
  failures.push(`Product FBA registry is invalid JSON: ${error.message}`);
}
if (registry) {
  if (registry.status !== "boundary_ready") {
    failures.push("Product FBA registry must remain boundary_ready before execution evidence");
  }
  const external = registry.external_transport ?? {};
  if (external.crate !== "rustok-product-transport") {
    failures.push("Product FBA registry must name rustok-product-transport");
  }
  if (external.status !== "runtime_wired_execution_pending") {
    failures.push("Product external transport must remain runtime_wired_execution_pending");
  }
  if (external.client !== "GrpcProductCatalogReadProvider") {
    failures.push("Product external transport client identity drift");
  }
  if (external.server !== "ProductCatalogGrpcService") {
    failures.push("Product external transport server identity drift");
  }
  const profiles = registry.contract_tests?.profiles ?? [];
  if (!profiles.includes("remote_adapter_placeholder") || !profiles.includes("grpc_loopback")) {
    failures.push("Product contract profiles must retain placeholder and add grpc_loopback");
  }
  for (const operation of [
    "read_product_projection",
    "read_variant_product_projection",
    "list_published_products",
  ]) {
    const testCase = registry.contract_tests?.cases?.find(
      (entry) => entry.operation === operation,
    );
    if (!testCase?.profiles?.includes("grpc_loopback")) {
      failures.push(`Product ${operation} contract case must include grpc_loopback`);
    }
    if (!testCase?.assertions?.includes("trusted_authority_replaces_actor")) {
      failures.push(`Product ${operation} contract case must assert trusted authority`);
    }
  }
}

requireAll(plan, [
  "`rustok-product-transport` supplies a concrete tonic gRPC client/server adapter",
  "Adapter and production-wiring source are complete",
  "run by the implementation agent",
  "remote-profile execution evidence remain open",
  "cargo test -p rustok-product-transport --test port_conformance",
  "ProductCatalogReadRuntime::external",
  "verify-product-catalog-grpc-transport.mjs",
], "Product implementation plan");

if (failures.length > 0) {
  console.error("Product catalog gRPC transport verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Product catalog gRPC transport source verification passed");
