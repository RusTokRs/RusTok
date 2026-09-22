#!/usr/bin/env node
// RusTok runtime-context invariant guardrails.
// This is intentionally source-level and fast: it catches drift in request
// context propagation without requiring a full Rust compile.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function readRepo(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) {
    fail(description);
  }
}

function functionBody(text, functionName) {
  const signature = new RegExp(`fn\\s+${functionName}\\s*\\(`);
  const match = signature.exec(text);
  if (!match) {
    fail(`missing function ${functionName}`);
    return "";
  }

  const openBrace = text.indexOf("{", match.index);
  if (openBrace === -1) {
    fail(`missing body for function ${functionName}`);
    return "";
  }

  let depth = 0;
  for (let index = openBrace; index < text.length; index += 1) {
    const char = text[index];
    if (char === "{") {
      depth += 1;
    } else if (char === "}") {
      depth -= 1;
      if (depth === 0) {
        return text.slice(openBrace, index + 1);
      }
    }
  }

  fail(`unterminated body for function ${functionName}`);
  return "";
}

function assertChannelRequestFacts() {
  const relativePath = "apps/server/src/middleware/channel.rs";
  const source = readRepo(relativePath);
  const factsBody = functionBody(source, "build_request_facts");
  const keyBody = functionBody(source, "channel_cache_key_from_facts");

  assertContains(
    source,
    /struct\s+ChannelCacheKey[\s\S]*oauth_app_id:\s*Option<Uuid>[\s\S]*locale:\s*Option<String>/,
    `${relativePath}: ChannelCacheKey must keep OAuth/client and locale dimensions`,
  );
  assertContains(
    factsBody,
    "get::<AuthContextExtension>()",
    `${relativePath}: RequestFacts.oauth_app_id must be read from AuthContextExtension`,
  );
  assertContains(
    factsBody,
    /and_then\(\|auth\|\s*auth\.0\.client_id\)/,
    `${relativePath}: RequestFacts.oauth_app_id must use auth client_id`,
  );
  assertContains(
    factsBody,
    "get::<ResolvedRequestLocale>()",
    `${relativePath}: RequestFacts.locale must be read from ResolvedRequestLocale`,
  );
  assertContains(
    factsBody,
    /effective_locale\.clone\(\)/,
    `${relativePath}: RequestFacts.locale must use the effective locale`,
  );
  assertContains(
    keyBody,
    /oauth_app_id:\s*facts\.oauth_app_id/,
    `${relativePath}: channel cache key must include RequestFacts.oauth_app_id`,
  );
  assertContains(
    keyBody,
    /locale:\s*facts\.locale\.as_deref\(\)\.map\(bounded_cache_component\)/,
    `${relativePath}: channel cache key must include RequestFacts.locale`,
  );
}

function assertRouterMiddlewareOrdering() {
  const relativePath = "apps/server/src/services/app_router.rs";
  const source = readRepo(relativePath);
  const routerBody = functionBody(source, "compose_application_router");
  const channelIndex = routerBody.lastIndexOf("middleware::channel::resolve");
  const authIndex = routerBody.lastIndexOf("middleware::auth_context::resolve_optional");
  const localeIndex = routerBody.lastIndexOf("middleware::locale::resolve_locale");
  const tenantIndex = routerBody.lastIndexOf("middleware::tenant::resolve");

  if (localeIndex === -1 || authIndex === -1 || channelIndex === -1 || tenantIndex === -1) {
    fail(`${relativePath}: expected tenant-bound locale/auth/channel middleware layers`);
    return;
  }

  // Axum executes layers from the bottom of the chain outward. In source order,
  // channel must therefore remain above auth_context, auth_context above locale,
  // and locale above the tenant resolver that establishes the tenant-bound chain.
  if (!(channelIndex < authIndex && authIndex < localeIndex && localeIndex < tenantIndex)) {
    fail(
      `${relativePath}: source order must keep channel above auth_context above locale above tenant so execution order is tenant -> locale -> auth_context -> channel`,
    );
  }
}

function assertLocaleCacheMetricNames() {
  const relativePath = "apps/server/src/controllers/metrics.rs";
  const source = readRepo(relativePath);
  const metricsBody = functionBody(source, "format_tenant_locale_cache_metrics");

  [
    "rustok_tenant_locale_cache_hits_total",
    "rustok_tenant_locale_cache_misses_total",
    "rustok_tenant_locale_db_queries_total",
    "rustok_tenant_locale_cache_invalidations_total",
    "rustok_tenant_locale_cache_entries",
  ].forEach((metricName) => {
    assertContains(
      metricsBody,
      metricName,
      `${relativePath}: missing tenant locale cache metric ${metricName}`,
    );
  });
}

function assertPagesDependencyEvidence() {
  const manifestPath = "modules.toml";
  const manifest = readRepo(manifestPath);
  assertContains(
    manifest,
    /pages\s*=\s*\{[^\n]*depends_on\s*=\s*\[[^\]]*"content"[^\]]*"page_builder"[^\]]*\]/,
    `${manifestPath}: pages must depend on both content and page_builder`,
  );

  const registryPath = "docs/modules/registry.md";
  const registry = readRepo(registryPath);
  assertContains(
    registry,
    /pages[\s\S]{0,240}content[\s\S]{0,240}page_builder/,
    `${registryPath}: pages dependency evidence must mention content and page_builder`,
  );
}

assertChannelRequestFacts();
assertRouterMiddlewareOrdering();
assertLocaleCacheMetricNames();
assertPagesDependencyEvidence();

if (failures.length > 0) {
  console.error("Runtime context invariant check failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Runtime context invariants passed");
