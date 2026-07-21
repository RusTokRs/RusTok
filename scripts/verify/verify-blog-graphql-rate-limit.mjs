#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function read(relativePath) {
  const target = path.join(repoRoot, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const policyPath = "crates/rustok-blog/src/graphql/rate_limit.rs";
const integrationPath = "crates/rustok-blog/tests/graphql_rate_limit_policy_test.rs";
const adapterPath = "apps/server/src/graphql/blog_rate_limit.rs";
const controllerPath = "apps/server/src/controllers/graphql.rs";
const evidencePath =
  "crates/rustok-blog/contracts/evidence/blog-graphql-rate-limit-runtime-harness.json";
const planPath = "crates/rustok-blog/docs/implementation-plan.md";

const policy = read(policyPath);
const integration = read(integrationPath);
const adapter = read(adapterPath);
const controller = read(controllerPath);
const plan = read(planPath);
let evidence = null;
try {
  evidence = JSON.parse(read(evidencePath));
} catch (error) {
  failures.push(`${evidencePath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  "BlogGraphqlRateLimitPolicy",
  "BlogGraphqlRateLimitExceeded",
  "BLOG_RATE_LIMITED",
  'ext.set("retryAfter", exceeded.retry_after as i64)',
  "rate_limited_error_response",
  "headers.insert(header::RETRY_AFTER, value)",
  ".http_headers(headers)",
  "BLOG_RATE_LIMIT_BACKEND_UNAVAILABLE",
]) {
  requireMarker(policy, marker, policyPath);
}

for (const marker of [
  "retry_after(&response)",
  'Some("9")',
  'Some("30")',
  "backend_unavailable_returns_fail_closed_graphql_error_without_retry_after",
  "selected_operation_keeps_document_wide_fail_closed_accounting",
]) {
  requireMarker(integration, marker, integrationPath);
}
const backendFailureTest = integration.match(
  /async fn backend_unavailable_returns_fail_closed_graphql_error_without_retry_after\(\)[\s\S]*?(?=\n#\[tokio::test\]|$)/,
)?.[0];
if (!backendFailureTest) {
  failures.push(`${integrationPath}: backend-unavailable test block is missing`);
} else {
  requireMarker(
    backendFailureTest,
    "assert_eq!(retry_after(&response), None);",
    `${integrationPath} backend-unavailable test`,
  );
  rejectMarker(
    backendFailureTest,
    "Some(\"",
    `${integrationPath} backend-unavailable test`,
  );
}

for (const marker of [
  "ServerBlogGraphqlRateLimiter",
  "RateLimitCheckError::Exceeded",
  "BlogGraphqlRateLimitExceeded",
  "RateLimitCheckError::BackendUnavailable",
]) {
  requireMarker(adapter, marker, adapterPath);
}

for (const marker of [
  ") -> Response",
  "graphql_http_response(response)",
  "let graphql_headers = response.http_headers.clone()",
  "response.headers_mut().extend(graphql_headers)",
  "graphql_http_response_preserves_extension_headers",
  "header::RETRY_AFTER",
  "header::CONTENT_TYPE",
]) {
  requireMarker(controller, marker, controllerPath);
}
rejectMarker(controller, ") -> Json<async_graphql::Response>", controllerPath);

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version must be 1`);
  if (evidence.module !== "blog" || evidence.surface !== "graphql_rate_limit") {
    failures.push(`${evidencePath}: module/surface identity drift`);
  }
  if (evidence.status !== "executable_no_compile") {
    failures.push(`${evidencePath}: status drift`);
  }
  if (evidence.compile_policy !== "not_run_by_request") {
    failures.push(`${evidencePath}: compile policy drift`);
  }
  if (evidence.production_contract?.policy !== policyPath) {
    failures.push(`${evidencePath}: policy path drift`);
  }
  if (evidence.production_contract?.host_adapter !== adapterPath) {
    failures.push(`${evidencePath}: adapter path drift`);
  }
  if (evidence.production_contract?.http_handoff !== controllerPath) {
    failures.push(`${evidencePath}: controller handoff path drift`);
  }
  for (const target of [integrationPath, adapterPath, controllerPath]) {
    if (!(evidence.test_targets ?? []).includes(target)) {
      failures.push(`${evidencePath}: missing test target ${target}`);
    }
  }
}

for (const marker of [
  "blog-graphql-rate-limit-runtime-harness.json",
  "Retry-After",
  "verify-blog-graphql-rate-limit.mjs",
]) {
  requireMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error("Blog GraphQL rate-limit verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Blog GraphQL rate-limit verification passed");
