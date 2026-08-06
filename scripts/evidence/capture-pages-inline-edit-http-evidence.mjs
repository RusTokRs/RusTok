#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-pages/contracts/evidence/pages-inline-edit-artifact-http-execution-contract.json";
const contract = JSON.parse(readFileSync(path.join(repoRoot, contractPath), "utf8"));
const requestTimeoutMs = 30_000;

function fail(message) {
  throw new Error(`Pages inline edit HTTP evidence capture failed: ${message}`);
}

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: capture-pages-inline-edit-http-evidence.mjs " +
          "--base-url URL --page-id UUID --locale LOCALE --output FILE " +
          "[--source-commit SHA]",
      );
      process.exit(0);
    }
    if (["--base-url", "--page-id", "--locale", "--output", "--source-commit"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) fail(`${argument} requires a value`);
      const key = argument
        .slice(2)
        .replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
      options[key] = value;
      index += 1;
      continue;
    }
    fail(`unknown argument ${argument}`);
  }
  return options;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function requireCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/u.test(value)) {
    fail(`${label} must be a lowercase 40-character git commit`);
  }
  return value;
}

function currentCommit() {
  return requireCommit(
    execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: repoRoot,
      encoding: "utf8",
    }).trim(),
    "git HEAD",
  );
}

function boundedSecretEnvironment(name, maximumLength) {
  const value = process.env[name];
  if (value === undefined || value === "") return null;
  if (
    typeof value !== "string" ||
    value.length > maximumLength ||
    /[\r\n\u0000]/u.test(value)
  ) {
    fail(`${name} is outside the bounded request credential input`);
  }
  return value;
}

function commonHeaders() {
  const name = "RUSTOK_PAGES_INLINE_EDIT_EVIDENCE_COMMON_HEADERS_JSON";
  const value = process.env[name];
  if (!value) return { headers: {}, environment_names: [] };
  if (value.length > 16_384 || /[\u0000]/u.test(value)) {
    fail(`${name} is outside the bounded common-header input`);
  }
  let parsed;
  try {
    parsed = JSON.parse(value);
  } catch (error) {
    fail(`${name} must contain a JSON object: ${error.message}`);
  }
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    fail(`${name} must contain a JSON object`);
  }
  const headers = {};
  for (const [headerName, headerValue] of Object.entries(parsed)) {
    const normalized = headerName.toLowerCase();
    if (!/^[a-z0-9!#$%&'*+.^_`|~-]+$/u.test(normalized)) {
      fail(`${name} contains an invalid header name`);
    }
    if (["authorization", "cookie", "set-cookie"].includes(normalized)) {
      fail(`${name} must not carry authorization or cookie headers`);
    }
    if (
      typeof headerValue !== "string" ||
      headerValue.length > 4096 ||
      /[\r\n\u0000]/u.test(headerValue)
    ) {
      fail(`${name}.${headerName} is outside the bounded header value input`);
    }
    headers[normalized] = headerValue;
  }
  return { headers, environment_names: [name] };
}

function requireBaseUrl(value) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail("--base-url must be an absolute HTTP(S) URL");
  }
  if (
    !["http:", "https:"].includes(parsed.protocol) ||
    parsed.username ||
    parsed.password ||
    parsed.search ||
    parsed.hash ||
    !["", "/"].includes(parsed.pathname)
  ) {
    fail("--base-url must be an HTTP(S) origin without credentials, path, query, or fragment");
  }
  return parsed.origin;
}

function requireUuid(value) {
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu.test(value)) {
    fail("--page-id must be a UUID");
  }
  return value.toLowerCase();
}

function requireLocale(value) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    value.length === 0 ||
    Buffer.byteLength(value, "utf8") > 64 ||
    /[\u0000-\u001f\u007f/\\?#]/u.test(value)
  ) {
    fail("--locale must be a non-empty bounded path-safe locale");
  }
  return value;
}

function selectedHeaders(headers) {
  const output = {};
  for (const name of [
    "cache-control",
    "content-type",
    "etag",
    "cross-origin-resource-policy",
    "x-robots-tag",
    "location",
  ]) {
    const value = headers.get(name);
    if (value !== null) output[name] = value;
  }
  return output;
}

async function request(url, options = {}) {
  let response;
  try {
    response = await fetch(url, {
      redirect: "manual",
      signal: AbortSignal.timeout(requestTimeoutMs),
      ...options,
    });
  } catch (error) {
    fail(`request failed for ${url}: ${error.message}`);
  }
  const body = Buffer.from(await response.arrayBuffer());
  return {
    status: response.status,
    headers: selectedHeaders(response.headers),
    body,
  };
}

function requireHeader(response, name, expected, label) {
  const actual = response.headers[name];
  if (actual !== expected) {
    fail(`${label} ${name} expected ${expected}, got ${actual ?? "<missing>"}`);
  }
}

function responseRecord(response) {
  return {
    status: response.status,
    headers: response.headers,
    body_bytes: response.body.length,
    body_sha256: sha256(response.body),
    raw_body_persisted: false,
  };
}

async function captureAsset(baseUrl, specification) {
  const url = new URL(specification.path, baseUrl).toString();
  const initial = await request(url);
  if (initial.status !== 200) {
    fail(`${specification.id} initial request expected 200, got ${initial.status}`);
  }
  requireHeader(initial, "content-type", specification.content_type, specification.id);
  requireHeader(
    initial,
    "cache-control",
    contract.http_capture.asset_cache_control,
    specification.id,
  );
  requireHeader(
    initial,
    "cross-origin-resource-policy",
    contract.http_capture.asset_cross_origin_resource_policy,
    specification.id,
  );
  const etag = initial.headers.etag;
  if (typeof etag !== "string" || !/^"[0-9a-f]{64}"$/u.test(etag)) {
    fail(`${specification.id} must return a full strong SHA-256 ETag`);
  }
  if (etag !== `"${sha256(initial.body)}"`) {
    fail(`${specification.id} ETag does not match the response body`);
  }

  const exact = await request(url, { headers: { "if-none-match": etag } });
  if (exact.status !== 304 || exact.body.length !== 0) {
    fail(`${specification.id} exact If-None-Match must return empty 304`);
  }
  requireHeader(exact, "etag", etag, `${specification.id} exact 304`);
  requireHeader(
    exact,
    "cache-control",
    contract.http_capture.asset_cache_control,
    `${specification.id} exact 304`,
  );
  requireHeader(
    exact,
    "cross-origin-resource-policy",
    contract.http_capture.asset_cross_origin_resource_policy,
    `${specification.id} exact 304`,
  );

  const weak = await request(url, { headers: { "if-none-match": `W/${etag}` } });
  if (weak.status !== 304 || weak.body.length !== 0) {
    fail(`${specification.id} weak If-None-Match must return empty 304`);
  }
  requireHeader(weak, "etag", etag, `${specification.id} weak 304`);

  return {
    id: specification.id,
    path: specification.path,
    initial: responseRecord(initial),
    exact_if_none_match: responseRecord(exact),
    weak_if_none_match: responseRecord(weak),
  };
}

function scenarioHeaders(scenario, shared) {
  if (scenario.id === "anonymous") {
    return { headers: { ...shared.headers }, environment_names: [...shared.environment_names] };
  }
  const authorization = scenario.authorization_env
    ? boundedSecretEnvironment(scenario.authorization_env, 8192)
    : null;
  const cookie = scenario.cookie_env
    ? boundedSecretEnvironment(scenario.cookie_env, 16_384)
    : null;
  if (!authorization && !cookie) {
    fail(
      `${scenario.id} requires ${scenario.authorization_env} or ${scenario.cookie_env}`,
    );
  }
  const headers = { ...shared.headers };
  const environmentNames = [...shared.environment_names];
  if (authorization) {
    headers.authorization = authorization;
    environmentNames.push(scenario.authorization_env);
  }
  if (cookie) {
    headers.cookie = cookie;
    environmentNames.push(scenario.cookie_env);
  }
  return { headers, environment_names: environmentNames.sort() };
}

async function captureAuthoringScenario(baseUrl, pageId, locale, scenario, shared) {
  const route = new URL(`/${encodeURIComponent(locale)}/modules/pages-authoring`, baseUrl);
  route.searchParams.set("page_id", pageId);
  const credentials = scenarioHeaders(scenario, shared);
  const response = await request(route.toString(), { headers: credentials.headers });
  if (response.status !== scenario.expected_status) {
    fail(`${scenario.id} expected ${scenario.expected_status}, got ${response.status}`);
  }
  requireHeader(
    response,
    "cache-control",
    contract.http_capture.authoring_route_cache_control,
    scenario.id,
  );
  requireHeader(
    response,
    "x-robots-tag",
    contract.http_capture.authoring_route_robots,
    scenario.id,
  );

  const markerChecks = {};
  const forbiddenChecks = {};
  if (scenario.id === "direct_user") {
    const html = response.body.toString("utf8");
    for (const marker of contract.http_capture.direct_user_required_markers) {
      markerChecks[marker] = html.includes(marker);
      if (!markerChecks[marker]) fail(`direct_user HTML is missing ${marker}`);
    }
    markerChecks.page_id = html.includes(pageId);
    markerChecks.locale = html.includes(locale);
    if (!markerChecks.page_id || !markerChecks.locale) {
      fail("direct_user HTML must bind the requested page id and exact locale");
    }
    const lower = html.toLowerCase();
    for (const marker of contract.http_capture.direct_user_forbidden_markers) {
      forbiddenChecks[marker] = lower.includes(marker.toLowerCase());
      if (forbiddenChecks[marker]) {
        fail(`direct_user HTML contains forbidden marker ${marker}`);
      }
    }
  }

  return {
    id: scenario.id,
    expected_status: scenario.expected_status,
    response: responseRecord(response),
    credential_environment_names: credentials.environment_names,
    credential_values_persisted: false,
    required_markers_present: markerChecks,
    forbidden_markers_present: forbiddenChecks,
  };
}

function writeAtomic(output, document) {
  const absolute = path.isAbsolute(output) ? path.resolve(output) : path.resolve(repoRoot, output);
  mkdirSync(path.dirname(absolute), { recursive: true });
  const temporary = `${absolute}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, absolute);
  return absolute;
}

const options = parseArguments(process.argv.slice(2));
for (const required of ["baseUrl", "pageId", "locale", "output"]) {
  if (!options[required]) fail(`--${required.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)} is required`);
}
const baseUrl = requireBaseUrl(options.baseUrl);
const pageId = requireUuid(options.pageId);
const locale = requireLocale(options.locale);
const head = currentCommit();
const sourceCommit = options.sourceCommit
  ? requireCommit(options.sourceCommit, "--source-commit")
  : head;
if (sourceCommit !== head) {
  fail(`--source-commit ${sourceCommit} does not match git HEAD ${head}`);
}

const shared = commonHeaders();
const assets = [];
for (const specification of contract.http_capture.asset_paths) {
  assets.push(await captureAsset(baseUrl, specification));
}
const authoring = [];
for (const scenario of contract.http_capture.authoring_scenarios) {
  authoring.push(await captureAuthoringScenario(baseUrl, pageId, locale, scenario, shared));
}

const document = {
  format: contract.http_capture.format,
  status: "passed",
  source_commit: sourceCommit,
  captured_at: new Date().toISOString(),
  target: {
    origin: baseUrl,
    page_id_shape: "uuid",
    locale,
  },
  assets,
  authoring,
  privacy: {
    credential_environment_names_only: true,
    credential_values_persisted: false,
    raw_response_bodies_persisted: false,
    grants_or_proofs_persisted: false,
  },
};

const output = writeAtomic(options.output, document);
console.log(
  `[capture-pages-inline-edit-http-evidence] PASS assets=${assets.length} scenarios=${authoring.length} output=${output}`,
);
