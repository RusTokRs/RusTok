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
    "crates/modules/rustok-product/ports/Cargo.toml",
    options.storageDependency
      ? `name = "rustok-product-ports"\nrustok-api.workspace = true\nthiserror.workspace = true\ntonic = { workspace = true }\ntonic-prost = { workspace = true, optional = true }\nurl.workspace = true\nprotoc-bin-vendored = { workspace = true, optional = true }\ntonic-prost-build = { workspace = true, optional = true }\ntokio-stream = { version = "0.1", features = ["net"] }\nsea-orm.workspace = true`
      : `name = "rustok-product-ports"\nrustok-api.workspace = true\nthiserror.workspace = true\ntonic = { workspace = true }\ntonic-prost = { workspace = true, optional = true }\nurl.workspace = true\nprotoc-bin-vendored = { workspace = true, optional = true }\ntonic-prost-build = { workspace = true, optional = true }\ntokio-stream = { version = "0.1", features = ["net"] }`,
  );
  write(
    root,
    "crates/modules/rustok-product/ports/build.rs",
    `protoc_bin_vendored::protoc_bin_path(); tonic_prost_build::configure(); "proto/rustok/product/product_catalog.proto";`,
  );
  const variantRpc = options.missingRpc
    ? ""
    : "rpc ReadVariantProductProjection(JsonRequest) returns (JsonResponse);";
  write(
    root,
    "crates/modules/rustok-product/ports/proto/rustok/product/product_catalog.proto",
    `package rustok.product; service ProductCatalogReadService { rpc ReadProductProjection(JsonRequest) returns (JsonResponse); ${variantRpc} rpc ListPublishedProducts(JsonRequest) returns (JsonResponse); } message JsonRequest { bytes context_json = 1; bytes input_json = 2; } message JsonResponse { bytes output_json = 1; }`,
  );
  write(
    root,
    "crates/modules/rustok-product/ports/src/grpc/mod.rs",
    `pub mod client; pub mod connection; pub mod server; tonic::include_proto!("rustok.product"); GrpcProductCatalogReadProvider GrpcProductCatalogReadConnectionConfig ProductCatalogGrpcService TrustedProductCatalogAuthority ProductCatalogGrpcOperation`,
  );
  write(
    root,
    "crates/modules/rustok-product/ports/src/grpc/client.rs",
    `impl ProductCatalogReadPort for GrpcProductCatalogReadProvider { async fn read_product_projection( context_json: encode(&context)? input_json: encode(&request)? ); async fn read_variant_product_projection( context_json: encode(&context)? input_json: encode(&request)? ); async fn list_published_products( context_json: encode(&context)? input_json: encode(&request)? ); } request.set_timeout(Duration::from_millis(deadline_ms)); serde_json::from_slice::<PortError>(status.details()); Code::DeadlineExceeded => PortErrorKind::Timeout; Code::Unavailable | Code::ResourceExhausted => PortErrorKind::Unavailable;`,
  );
  write(
    root,
    "crates/modules/rustok-product/ports/src/grpc/server.rs",
    options.missingAuthority
      ? `impl<P> ProductCatalogReadService for ProductCatalogGrpcService<P> where P: ProductCatalogReadPort + 'static { }`
      : `impl<P> ProductCatalogReadService for ProductCatalogGrpcService<P> where P: ProductCatalogReadPort + 'static { } TrustedProductCatalogAuthority allowed_operations: HashSet<ProductCatalogGrpcOperation> ReadProductProjection ReadVariantProductProjection ListPublishedProducts trusted_context( claimed.tenant_id != authority.tenant_id claimed.actor = authority.actor.clone() claimed.claims.clone_from(&authority.claims) claimed.roles.clone_from(&authority.roles) Status::with_details(code, error.message, Bytes::from(details)) Status::unauthenticated("trusted Product catalog authority is missing") assert_eq!(trusted.actor, PortActor::service("trusted-product-service"))`,
  );
  write(
    root,
    "crates/modules/rustok-product/tests/port_conformance.rs",
    `impl ProductCatalogReadPort for MockProductCatalogReadPort ProductCatalogReadServiceServer::with_interceptor GrpcProductCatalogReadProvider::connect ProductCatalogGrpcOperation::ReadProductProjection ProductCatalogGrpcOperation::ReadVariantProductProjection ProductCatalogGrpcOperation::ListPublishedProducts read_product_projection( read_variant_product_projection( list_published_products( product.product_not_found port.deadline_required PortActor::service("trusted-product-catalog-conformance") serve_with_incoming_shutdown`,
  );
  write(
    root,
    "crates/modules/rustok-product/ports/README.md",
    `rustok-product-ports ProductCatalogReadPort ProductVariantIdentityReadPort ProductCatalogCommandPort grpc-client grpc-server`,
  );
  const falsePromotion = options.falsePromotion === true;
  write(
    root,
    "crates/modules/rustok-product/contracts/product-fba-registry.json",
    JSON.stringify({
      status: falsePromotion ? "transport_verified" : "boundary_ready",
      evidence: {
        grpc_transport_verifier:
          "scripts/verify/verify-product-catalog-grpc-transport.mjs",
      },
      external_transport: {
        crate: "rustok-product-ports",
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
    "crates/modules/rustok-product/docs/implementation-plan.md",
    options.omitPlan
      ? "Product plan"
      : "The current transport lives in `rustok-product-ports`. cargo test -p rustok-product --test port_conformance ProductCatalogReadRuntime::external verify-product-catalog-grpc-transport.mjs",
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
