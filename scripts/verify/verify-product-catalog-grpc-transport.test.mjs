#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-catalog-grpc-transport.mjs",
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-grpc-transport-"));
  write(
    root,
    "crates/rustok-product-transport/Cargo.toml",
    options.storageDependency
      ? `name = "rustok-product-transport"\nrustok-api.workspace = true\nrustok-product.workspace = true\nthiserror.workspace = true\ntonic = { workspace = true }\ntonic-prost.workspace = true\nurl.workspace = true\nprotoc-bin-vendored.workspace = true\ntonic-prost-build.workspace = true\ntokio-stream = { version = "0.1", features = ["net"] }\nsea-orm.workspace = true`
      : `name = "rustok-product-transport"\nrustok-api.workspace = true\nrustok-product.workspace = true\nthiserror.workspace = true\ntonic = { workspace = true }\ntonic-prost.workspace = true\nurl.workspace = true\nprotoc-bin-vendored.workspace = true\ntonic-prost-build.workspace = true\ntokio-stream = { version = "0.1", features = ["net"] }`,
  );
  write(
    root,
    "crates/rustok-product-transport/build.rs",
    `protoc_bin_vendored::protoc_bin_path(); tonic_prost_build::configure(); "proto/rustok/product/product_catalog.proto";`,
  );
  const variantRpc = options.missingRpc
    ? ""
    : "rpc ReadVariantProductProjection(JsonRequest) returns (JsonResponse);";
  write(
    root,
    "crates/rustok-product-transport/proto/rustok/product/product_catalog.proto",
    `package rustok.product; service ProductCatalogReadService { rpc ReadProductProjection(JsonRequest) returns (JsonResponse); ${variantRpc} rpc ListPublishedProducts(JsonRequest) returns (JsonResponse); } message JsonRequest { bytes context_json = 1; bytes input_json = 2; } message JsonResponse { bytes output_json = 1; }`,
  );
  write(
    root,
    "crates/rustok-product-transport/src/lib.rs",
    `pub mod client; pub mod connection; pub mod server; tonic::include_proto!("rustok.product"); GrpcProductCatalogReadProvider GrpcProductCatalogReadConnectionConfig ProductCatalogGrpcService TrustedProductCatalogAuthority ProductCatalogGrpcOperation`,
  );
  write(
    root,
    "crates/rustok-product-transport/src/client.rs",
    `impl ProductCatalogReadPort for GrpcProductCatalogReadProvider { async fn read_product_projection( context_json: encode(&context)? input_json: encode(&request)? ); async fn read_variant_product_projection( context_json: encode(&context)? input_json: encode(&request)? ); async fn list_published_products( context_json: encode(&context)? input_json: encode(&request)? ); } request.set_timeout(Duration::from_millis(deadline_ms)); serde_json::from_slice::<PortError>(status.details()); Code::DeadlineExceeded => PortErrorKind::Timeout; Code::Unavailable | Code::ResourceExhausted => PortErrorKind::Unavailable;`,
  );
  write(
    root,
    "crates/rustok-product-transport/src/server.rs",
    options.missingAuthority
      ? `impl<P> ProductCatalogReadService for ProductCatalogGrpcService<P> where P: ProductCatalogReadPort + 'static { }`
      : `impl<P> ProductCatalogReadService for ProductCatalogGrpcService<P> where P: ProductCatalogReadPort + 'static { } TrustedProductCatalogAuthority allowed_operations: HashSet<ProductCatalogGrpcOperation> ReadProductProjection ReadVariantProductProjection ListPublishedProducts trusted_context( claimed.tenant_id != authority.tenant_id claimed.actor = authority.actor.clone() claimed.claims.clone_from(&authority.claims) claimed.roles.clone_from(&authority.roles) Status::with_details(code, error.message, Bytes::from(details)) Status::unauthenticated("trusted Product catalog authority is missing") assert_eq!(trusted.actor, PortActor::service("trusted-product-service"))`,
  );
  write(
    root,
    "crates/rustok-product-transport/tests/port_conformance.rs",
    `impl ProductCatalogReadPort for MockProductCatalogReadPort ProductCatalogReadServiceServer::with_interceptor GrpcProductCatalogReadProvider::connect ProductCatalogGrpcOperation::ReadProductProjection ProductCatalogGrpcOperation::ReadVariantProductProjection ProductCatalogGrpcOperation::ListPublishedProducts read_product_projection( read_variant_product_projection( list_published_products( product.product_not_found port.deadline_required PortActor::service("trusted-product-catalog-conformance") serve_with_incoming_shutdown`,
  );
  write(
    root,
    "crates/rustok-product-transport/README.md",
    `Typed tonic gRPC framing does not own Product DTOs TrustedProductCatalogAuthority cargo test -p rustok-product-transport --test port_conformance does not claim this command was executed boundary_ready`,
  );
  const falsePromotion = options.falsePromotion === true;
  write(
    root,
    "crates/rustok-product/contracts/product-fba-registry.json",
    JSON.stringify({
      status: falsePromotion ? "transport_verified" : "boundary_ready",
      evidence: {
        grpc_transport_verifier:
          "scripts/verify/verify-product-catalog-grpc-transport.mjs",
      },
      external_transport: {
        crate: "rustok-product-transport",
        client: "GrpcProductCatalogReadProvider",
        server: "ProductCatalogGrpcService",
        status: falsePromotion
          ? "transport_verified"
          : "runtime_wired_execution_pending",
      },
      contract_tests: {
        profiles: ["in_process", "remote_adapter_placeholder", "grpc_loopback"],
        cases: [
          {
            operation: "read_product_projection",
            profiles: ["in_process", "remote_adapter_placeholder", "grpc_loopback"],
            assertions: ["typed_port_error_mapping", "context_deadline_preserved", "trusted_authority_replaces_actor"],
          },
          {
            operation: "read_variant_product_projection",
            profiles: ["in_process", "remote_adapter_placeholder", "grpc_loopback"],
            assertions: ["typed_port_error_mapping", "context_deadline_preserved", "trusted_authority_replaces_actor"],
          },
          {
            operation: "list_published_products",
            profiles: ["in_process", "remote_adapter_placeholder", "grpc_loopback"],
            assertions: ["typed_port_error_mapping", "context_deadline_preserved", "trusted_authority_replaces_actor"],
          },
        ],
      },
    }),
  );
  write(
    root,
    "crates/rustok-product/docs/implementation-plan.md",
    options.omitPlan
      ? "Product plan"
      : "`rustok-product-transport` supplies a concrete tonic gRPC client/server adapter. Adapter and production-wiring source are complete, but neither path has been run by the implementation agent. Loopback and configured remote-profile execution evidence remain open. cargo test -p rustok-product-transport --test port_conformance ProductCatalogReadRuntime::external verify-product-catalog-grpc-transport.mjs",
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
    assert.notEqual(result.status, 0, "expected gRPC transport mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("Product catalog gRPC transport guard accepts canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("gRPC transport guard rejects missing owner RPC", () => {
  reject({ missingRpc: true }, /Product catalog protobuf/);
});

test("gRPC transport guard rejects missing trusted authority", () => {
  reject({ missingAuthority: true }, /gRPC server adapter/);
});

test("gRPC transport guard rejects Product storage ownership", () => {
  reject({ storageDependency: true }, /transport Cargo ownership/);
});

test("gRPC transport guard rejects false transport promotion", () => {
  reject({ falsePromotion: true }, /remain boundary_ready|runtime_wired_execution_pending/);
});

test("gRPC transport guard rejects missing plan handoff", () => {
  reject({ omitPlan: true }, /Product implementation plan/);
});
