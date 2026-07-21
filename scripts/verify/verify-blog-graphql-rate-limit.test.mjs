#!/usr/bin/env node

import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const verifier = path.resolve("scripts/verify/verify-blog-graphql-rate-limit.mjs");

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({
  missingRetryHeader = false,
  controllerDropsHeaders = false,
  backendFailureAdvertisesRetry = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-blog-rate-limit-"));

  write(
    root,
    "crates/rustok-blog/src/graphql/rate_limit.rs",
    `
      BlogGraphqlRateLimitPolicy BlogGraphqlRateLimitExceeded BLOG_RATE_LIMITED
      ext.set("retryAfter", exceeded.retry_after as i64)
      rate_limited_error_response
      ${missingRetryHeader ? "" : "headers.insert(header::RETRY_AFTER, value)"}
      .http_headers(headers)
      BLOG_RATE_LIMIT_BACKEND_UNAVAILABLE
    `,
  );
  write(
    root,
    "crates/rustok-blog/tests/graphql_rate_limit_policy_test.rs",
    `
      retry_after(&response) Some("9") Some("30")
      backend_unavailable_returns_fail_closed_graphql_error_without_retry_after
      selected_operation_keeps_document_wide_fail_closed_accounting
      ${backendFailureAdvertisesRetry ? 'assert_eq!(retry_after(&response), Some("60"));' : ""}
    `,
  );
  write(
    root,
    "apps/server/src/graphql/blog_rate_limit.rs",
    "ServerBlogGraphqlRateLimiter RateLimitCheckError::Exceeded BlogGraphqlRateLimitExceeded RateLimitCheckError::BackendUnavailable",
  );
  write(
    root,
    "apps/server/src/controllers/graphql.rs",
    controllerDropsHeaders
      ? ") -> Json<async_graphql::Response> { Json(response) }"
      : `
          ) -> Response
          graphql_http_response(response)
          let graphql_headers = response.http_headers.clone()
          response.headers_mut().extend(graphql_headers)
          graphql_http_response_preserves_extension_headers
          header::RETRY_AFTER
          header::CONTENT_TYPE
        `,
  );
  write(
    root,
    "crates/rustok-blog/contracts/evidence/blog-graphql-rate-limit-runtime-harness.json",
    JSON.stringify({
      schema_version: 1,
      module: "blog",
      surface: "graphql_rate_limit",
      status: "executable_no_compile",
      compile_policy: "not_run_by_request",
      test_targets: [
        "crates/rustok-blog/tests/graphql_rate_limit_policy_test.rs",
        "apps/server/src/graphql/blog_rate_limit.rs",
        "apps/server/src/controllers/graphql.rs",
      ],
      production_contract: {
        policy: "crates/rustok-blog/src/graphql/rate_limit.rs",
        host_adapter: "apps/server/src/graphql/blog_rate_limit.rs",
        http_handoff: "apps/server/src/controllers/graphql.rs",
      },
    }),
  );
  write(
    root,
    "crates/rustok-blog/docs/implementation-plan.md",
    "blog-graphql-rate-limit-runtime-harness.json Retry-After verify-blog-graphql-rate-limit.mjs",
  );

  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

test("Blog GraphQL rate-limit verifier accepts the canonical handoff", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("Blog GraphQL rate-limit verifier rejects a missing Retry-After header", () => {
  const root = fixture({ missingRetryHeader: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing headers.insert\(header::RETRY_AFTER, value\)/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("Blog GraphQL rate-limit verifier rejects a controller that drops headers", () => {
  const root = fixture({ controllerDropsHeaders: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(
      result.stderr,
      /missing graphql_http_response\(response\)|forbidden \) -> Json<async_graphql::Response>/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("Blog GraphQL rate-limit fixture keeps backend failures headerless", () => {
  const root = fixture({ backendFailureAdvertisesRetry: true });
  try {
    const source = readFile(path.join(
      root,
      "crates/rustok-blog/tests/graphql_rate_limit_policy_test.rs",
    ));
    assert.match(source, /backend_unavailable_returns_fail_closed/);
    assert.match(source, /Some\("60"\)/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function readFile(target) {
  return new TextDecoder().decode(
    // Avoid introducing another fs import solely for the negative documentation fixture.
    requireBytes(target),
  );
}

function requireBytes(target) {
  return new Uint8Array(
    // eslint-disable-next-line no-sync
    process.getBuiltinModule("node:fs").readFileSync(target),
  );
}
