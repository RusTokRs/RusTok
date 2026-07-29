#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-remote-consumer-behavior.mjs",
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-remote-consumers-"));

  write(
    root,
    "crates/rustok-commerce/Cargo.toml",
    `rustok-product-transport = { path = "../rustok-product-transport" }\ntokio-stream = { version = "0.1", features = ["net"] }\ntonic.workspace = true\n[[test]]\nname = "product_remote_consumer_behavior"\npath = "tests/product_remote_consumer_behavior.rs"`,
  );
  const commerceTimeout = options.missingCommerceTimeout
    ? ""
    : `"product.remote_timeout" remote_product_timeout_blocks_checkout_without_snapshot_fallback`;
  const fakeFallback = options.cartSnapshotFallback
    ? "fallback_to_cart_snapshot"
    : "";
  write(
    root,
    "crates/rustok-commerce/tests/product_remote_consumer_behavior.rs",
    `ProductCatalogReadServiceServer::with_interceptor ProductCatalogGrpcService::new GrpcProductCatalogReadProvider::connect ProductCatalogReadRuntime::external ProductCatalogReadProfile::External CheckoutPlanBuilder::new .build(tenant_id, actor_id, Uuid::new_v4(), &input, &snapshot) CheckoutError::BoundaryFailure assert_eq!(stage, "read_checkout_product_projection") "product.remote_unavailable" ${commerceTimeout} assert!(retryable) remote_product_unavailable_blocks_checkout_without_snapshot_fallback ${fakeFallback}`,
  );
  write(
    root,
    "crates/rustok-commerce/src/services/checkout_plan_builder.rs",
    `self.product_catalog_read_port .read_product_projection( .read_variant_product_projection( boundary_error("read_checkout_product_projection", error)`,
  );

  write(
    root,
    "crates/rustok-ai/Cargo.toml",
    `rustok-product-transport = { path = "../rustok-product-transport" }\ntokio-stream = { version = "0.1", features = ["net"] }\ntonic.workspace = true`,
  );
  const aiReview = options.missingAiReview ? "" : `"review_required": true`;
  const aiTimeout = options.missingAiTimeout
    ? ""
    : `"product.remote_timeout" remote_product_timeout_degrades_ai_enrichment`;
  write(
    root,
    "crates/rustok-ai/src/direct_product_attributes.rs",
    `mod remote_profile_tests ProductCatalogReadServiceServer::with_interceptor ProductCatalogGrpcService::new GrpcProductCatalogReadProvider::connect ProductCatalogReadRuntime::external ProductCatalogReadProfile::External runtime_with_remote_failure remote_product_unavailable_degrades_ai_enrichment ${aiTimeout} "product.remote_unavailable" assert_eq!(metadata["source"], "degraded") assert_eq!(metadata["catalog_enrichment"], "skipped") assert_eq!(metadata["errors"][0]["retryable"], true) ${aiReview} "persistence": "none"`,
  );

  const falsePromotion = options.falsePromotion === true;
  const staleStatus = options.staleStatus === true;
  write(
    root,
    "crates/rustok-product/contracts/product-fba-registry.json",
    JSON.stringify({
      status: falsePromotion ? "transport_verified" : "boundary_ready",
      evidence: {
        remote_consumer_behavior_verifier:
          "scripts/verify/verify-product-remote-consumer-behavior.mjs",
      },
      external_transport: {
        status: "runtime_wired_execution_pending",
      },
      remote_consumer_behavior: {
        status: staleStatus
          ? "executed"
          : "source_complete_execution_pending",
        commerce_test:
          "crates/rustok-commerce/tests/product_remote_consumer_behavior.rs",
        ai_source_test: "crates/rustok-ai/src/direct_product_attributes.rs",
        failure_profiles: ["unavailable", "timeout"],
        assertions: [
          "commerce_hard_dependency_no_cart_snapshot_fallback",
          "commerce_typed_boundary_error_preserved",
          "ai_catalog_enrichment_skipped",
          "ai_typed_degraded_error_preserved",
          "ai_operator_review_required",
          "ai_persistence_none",
        ],
      },
    }),
  );
  write(
    root,
    "crates/rustok-product/docs/implementation-plan.md",
    options.missingPlan
      ? "Product plan"
      : "Remote consumer behavior is now source-complete through executable loopback harnesses. Commerce it never substitutes the cart line snapshot for current Product authority. AI both failures skip catalog enrichment, requires operator review, and performs no persistence. Product remains `boundary_ready`. Add executable Commerce hard-dependency and AI degraded-behavior gRPC harnesses. Execute the Commerce and AI remote consumer behavior harnesses. cargo test -p rustok-commerce --test product_remote_consumer_behavior cargo test -p rustok-ai --features server --lib remote_product_ verify-product-remote-consumer-behavior.mjs",
  );

  const aiProductProfiles = options.missingGrpcProfile
    ? ["in_process", "remote_adapter_placeholder"]
    : ["in_process", "remote_adapter_placeholder", "grpc_loopback"];
  write(
    root,
    "crates/rustok-ai-product/contracts/ai-product-fba-registry.json",
    JSON.stringify({
      status: "boundary_ready",
      provider_dependencies: [
        {
          module: "product",
          required_profiles: aiProductProfiles,
        },
      ],
      evidence: {
        remote_consumer_behavior_verifier:
          "scripts/verify/verify-product-remote-consumer-behavior.mjs",
      },
      remote_consumer_behavior: {
        status: options.staleAiStatus
          ? "runtime_verified"
          : "source_complete_execution_pending",
        profile: "grpc_loopback",
        source: "crates/rustok-ai/src/direct_product_attributes.rs",
        failure_profiles: ["unavailable", "timeout"],
        assertions: [
          "generate_from_prompt_only",
          "skip_catalog_enrichment",
          "require_operator_review",
          "persistence_none",
          "typed_port_error_preserved",
        ],
      },
    }),
  );
  write(
    root,
    "crates/rustok-ai-product/docs/implementation-plan.md",
    options.missingAiProductPlan
      ? "AI-product plan"
      : "A source-complete gRPC loopback harness now exercises the same product-context function. Remote `Unavailable` and `Timeout` errors preserve typed behavior. The production result remains review-required and non-persistent. source_complete_execution_pending. Execute the remote Product consumer harness. cargo test -p rustok-ai --features server --lib remote_product_. verify-product-remote-consumer-behavior.mjs",
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
    assert.notEqual(result.status, 0, "expected remote consumer mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("remote consumer behavior guard accepts canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("guard rejects missing Commerce timeout behavior", () => {
  reject({ missingCommerceTimeout: true }, /Commerce remote Product hard-dependency harness/);
});

test("guard rejects cart snapshot fallback", () => {
  reject({ cartSnapshotFallback: true }, /Commerce remote Product fallback ownership/);
});

test("guard rejects missing AI timeout degradation", () => {
  reject({ missingAiTimeout: true }, /AI remote Product degraded-behavior harness/);
});

test("guard rejects missing AI operator review", () => {
  reject({ missingAiReview: true }, /AI remote Product degraded-behavior harness/);
});

test("guard rejects stale executed status without evidence", () => {
  reject({ staleStatus: true }, /source_complete_execution_pending/);
});

test("guard rejects false Product promotion", () => {
  reject({ falsePromotion: true }, /remain boundary_ready/);
});

test("guard rejects missing AI-product gRPC profile", () => {
  reject({ missingGrpcProfile: true }, /must include grpc_loopback/);
});

test("guard rejects false AI-product runtime evidence", () => {
  reject({ staleAiStatus: true }, /AI-product remote behavior must remain/);
});

test("guard rejects missing Product implementation-plan handoff", () => {
  reject({ missingPlan: true }, /Product remote consumer implementation plan/);
});

test("guard rejects missing AI-product implementation-plan handoff", () => {
  reject({ missingAiProductPlan: true }, /AI-product remote consumer implementation plan/);
});
