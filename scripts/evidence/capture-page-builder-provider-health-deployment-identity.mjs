#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = path.join(
  repoRoot,
  "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-identity-source.json",
);
const MAX_INVENTORY_BYTES = 1024 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;

function fail(message) {
  throw new Error(`Page Builder provider-health deployment identity capture failed: ${message}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: capture-page-builder-provider-health-deployment-identity.mjs " +
          "--inventory FILE --deployment-image-digest REPOSITORY@sha256:DIGEST " +
          "[--source-commit SHA] [--output FILE]",
      );
      process.exit(0);
    }
    if (
      ["--inventory", "--deployment-image-digest", "--source-commit", "--output"].includes(
        argument,
      )
    ) {
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

function requireCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/u.test(value)) {
    fail(`${label} must be a lowercase 40-character git commit`);
  }
  return value;
}

function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
    shell: false,
    maxBuffer: 1024 * 1024,
  });
  if (result.error) fail(`git HEAD lookup failed: ${result.error.message}`);
  if (result.status !== 0) fail("git HEAD lookup returned a non-zero status");
  return requireCommit(result.stdout.trim(), "git HEAD");
}

function requireDeploymentImageDigest(value) {
  if (
    typeof value !== "string" ||
    value.length > 512 ||
    /[\s\u0000-\u001f\u007f]/u.test(value) ||
    value.includes("://")
  ) {
    fail("--deployment-image-digest must be a bounded Docker RepoDigest");
  }
  const parts = value.split("@");
  if (
    parts.length !== 2 ||
    parts[0].length === 0 ||
    !/^sha256:[0-9a-f]{64}$/u.test(parts[1])
  ) {
    fail("--deployment-image-digest must be REPOSITORY@sha256:DIGEST");
  }
  return value;
}

function requireBoundedIdentifier(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    Buffer.byteLength(value) > 128 ||
    !/^[A-Za-z0-9._:/-]+$/u.test(value)
  ) {
    fail(`${label} must be a bounded deployment identifier`);
  }
  return value;
}

function requireMetricsUrl(value, label) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail(`${label} must be an absolute HTTP(S) URL`);
  }
  if (
    !["http:", "https:"].includes(parsed.protocol) ||
    parsed.username ||
    parsed.password ||
    parsed.search ||
    parsed.hash
  ) {
    fail(`${label} must be an HTTP(S) URL without credentials, query, or fragment`);
  }
  return parsed.href;
}

function regularJsonFile(inputPath, label, maximumBytes) {
  const absolute = path.isAbsolute(inputPath)
    ? path.resolve(inputPath)
    : path.resolve(process.cwd(), inputPath);
  if (!existsSync(absolute)) fail(`${label} is missing`);
  const link = lstatSync(absolute);
  if (link.isSymbolicLink() || !link.isFile()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const stats = statSync(absolute);
  if (stats.size <= 0 || stats.size > maximumBytes) {
    fail(`${label} is outside the bounded size`);
  }
  let parsed;
  try {
    parsed = JSON.parse(readFileSync(absolute, "utf8"));
  } catch (error) {
    fail(`${label} must contain JSON: ${error.message}`);
  }
  return parsed;
}

function requireInventory(inputPath) {
  const inventory = regularJsonFile(inputPath, "inventory", MAX_INVENTORY_BYTES);
  if (inventory === null || typeof inventory !== "object" || Array.isArray(inventory)) {
    fail("inventory must be a JSON object");
  }
  if (inventory.schema_version !== 1) fail("inventory schema_version must be 1");
  const deploymentId = requireBoundedIdentifier(inventory.deployment_id, "deployment_id");
  if (inventory.inventory_complete !== true) {
    fail("inventory_complete must be true before deployment identity can be captured");
  }
  if (
    !Array.isArray(inventory.targets) ||
    inventory.targets.length === 0 ||
    inventory.targets.length > 64
  ) {
    fail("inventory targets must contain between 1 and 64 expected targets");
  }

  const targetIds = new Set();
  const metricsUrls = new Set();
  const targets = inventory.targets.map((target, index) => {
    if (target === null || typeof target !== "object" || Array.isArray(target)) {
      fail(`targets[${index}] must be an object`);
    }
    const targetId = requireBoundedIdentifier(target.target_id, `targets[${index}].target_id`);
    const metricsUrl = requireMetricsUrl(target.metrics_url, `targets[${index}].metrics_url`);
    if (targetIds.has(targetId)) fail(`duplicate target_id ${targetId}`);
    if (metricsUrls.has(metricsUrl)) fail(`duplicate metrics_url for target ${targetId}`);
    targetIds.add(targetId);
    metricsUrls.add(metricsUrl);
    return { target_id: targetId, metrics_url: metricsUrl };
  });

  return { deployment_id: deploymentId, targets };
}

function boundedSecretEnvironment(name, maximumLength) {
  const value = process.env[name];
  if (value === undefined || value === "") return null;
  if (value.length > maximumLength || /[\r\n\u0000]/u.test(value)) {
    fail(`${name} is outside the bounded credential input`);
  }
  return value;
}

function requestHeaders(contract) {
  const environmentNames = [];
  const headers = {
    accept: "text/plain",
    "cache-control": "no-cache",
    pragma: "no-cache",
  };

  const commonName = contract.capture.credential_environment.common_headers_json;
  const common = process.env[commonName];
  if (common) {
    if (common.length > 16_384 || /[\u0000]/u.test(common)) {
      fail(`${commonName} is outside the bounded common-header input`);
    }
    let parsed;
    try {
      parsed = JSON.parse(common);
    } catch (error) {
      fail(`${commonName} must contain a JSON object: ${error.message}`);
    }
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
      fail(`${commonName} must contain a JSON object`);
    }
    for (const [headerName, headerValue] of Object.entries(parsed)) {
      const normalized = headerName.toLowerCase();
      if (!/^[a-z0-9!#$%&'*+.^_`|~-]+$/u.test(normalized)) {
        fail(`${commonName} contains invalid header ${headerName}`);
      }
      if (["authorization", "cookie", "host", "content-length"].includes(normalized)) {
        fail(`${commonName} must not contain credential or framing headers`);
      }
      if (
        typeof headerValue !== "string" ||
        headerValue.length > 4096 ||
        /[\r\n\u0000]/u.test(headerValue)
      ) {
        fail(`${commonName}.${headerName} is outside the bounded header input`);
      }
      headers[normalized] = headerValue;
    }
    environmentNames.push(commonName);
  }

  const authorizationName = contract.capture.credential_environment.authorization;
  const authorization = boundedSecretEnvironment(authorizationName, 8192);
  if (authorization) {
    headers.authorization = authorization;
    environmentNames.push(authorizationName);
  }

  const cookieName = contract.capture.credential_environment.cookie;
  const cookie = boundedSecretEnvironment(cookieName, 16_384);
  if (cookie) {
    headers.cookie = cookie;
    environmentNames.push(cookieName);
  }

  return { headers, environment_names: [...new Set(environmentNames)].sort() };
}

function regularSourceHash(relativePath) {
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail(`source file ${relativePath} escapes repository root`);
  }
  if (!existsSync(absolute)) fail(`source file ${relativePath} is missing`);
  const link = lstatSync(absolute);
  if (link.isSymbolicLink() || !link.isFile()) {
    fail(`source file ${relativePath} must be a regular non-symlink file`);
  }
  const stats = statSync(absolute);
  if (stats.size <= 0 || stats.size > MAX_SOURCE_BYTES) {
    fail(`source file ${relativePath} is outside the bounded source size`);
  }
  return sha256(readFileSync(absolute));
}

function sourceHashes(contract) {
  if (
    !Array.isArray(contract.required_source_files) ||
    contract.required_source_files.length === 0 ||
    contract.required_source_files.length > 64
  ) {
    fail("required source-file set is outside the bounded contract");
  }
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => [relativePath, regularSourceHash(relativePath)]),
  );
}

function requireBuildInfo(metricsBody, sourceCommit, targetId) {
  const lines = metricsBody.toString("utf8").split(/\r?\n/u);
  const matches = [];
  const pattern =
    /^rustok_page_builder_provider_build_info\{source_commit="([0-9A-Fa-f]{40})"\}\s+([^\s]+)(?:\s+\d+)?$/u;
  for (const line of lines) {
    const match = pattern.exec(line.trim());
    if (!match) continue;
    const value = Number(match[2]);
    if (!Number.isFinite(value)) fail(`${targetId} build-info value is not finite`);
    matches.push({ source_commit: match[1].toLowerCase(), value });
  }
  if (matches.length !== 1) {
    fail(`${targetId} must expose exactly one Page Builder provider build-info series`);
  }
  if (matches[0].value !== 1) fail(`${targetId} build-info series must equal 1`);
  if (matches[0].source_commit !== sourceCommit) {
    fail(
      `${targetId} reports source commit ${matches[0].source_commit}, expected ${sourceCommit}`,
    );
  }
  return matches[0].source_commit;
}

async function captureTarget(target, contract, sourceCommit, credentials) {
  let response;
  try {
    response = await fetch(target.metrics_url, {
      method: "GET",
      headers: credentials.headers,
      redirect: "manual",
      signal: AbortSignal.timeout(contract.capture.request_timeout_ms),
    });
  } catch (error) {
    fail(`${target.target_id} metrics request failed: ${error.message}`);
  }
  if (response.status !== 200) {
    fail(`${target.target_id} metrics request expected 200, got ${response.status}`);
  }
  const body = Buffer.from(await response.arrayBuffer());
  if (body.byteLength <= 0 || body.byteLength > contract.capture.maximum_metrics_body_bytes) {
    fail(`${target.target_id} metrics response is outside the bounded body size`);
  }
  const reportedSourceCommit = requireBuildInfo(body, sourceCommit, target.target_id);
  return {
    target_id: target.target_id,
    metrics_url_bytes: Buffer.byteLength(target.metrics_url),
    metrics_url_sha256: sha256(target.metrics_url),
    raw_metrics_url_persisted: false,
    status: response.status,
    response_bytes: body.byteLength,
    response_sha256: sha256(body),
    raw_response_persisted: false,
    reported_source_commit: reportedSourceCommit,
    source_commit_verified_equal_checkout: true,
  };
}

function outputPath(contract, requested) {
  const candidate = requested ?? contract.capture.default_output;
  if (typeof candidate !== "string" || candidate.length === 0 || candidate.length > 16_384) {
    fail("output path is invalid");
  }
  const absolute = path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("deployment identity output must remain inside repository target/");
  }
  return absolute;
}

function writeAtomic(location, document) {
  mkdirSync(path.dirname(location), { recursive: true });
  const temporary = `${location}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, location);
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (!options.inventory) fail("--inventory is required");
  if (!options.deploymentImageDigest) fail("--deployment-image-digest is required");

  const contract = JSON.parse(readFileSync(contractPath, "utf8"));
  if (contract.status !== "source_ready_execution_pending") {
    fail("deployment identity source contract must not claim execution before capture starts");
  }

  const head = currentCommit();
  const sourceCommit = options.sourceCommit
    ? requireCommit(options.sourceCommit, "--source-commit")
    : head;
  if (sourceCommit !== head) {
    fail(`source commit ${sourceCommit} does not match git HEAD ${head}`);
  }
  const deploymentImageDigest = requireDeploymentImageDigest(options.deploymentImageDigest);
  const inventory = requireInventory(options.inventory);
  const credentials = requestHeaders(contract);
  const output = outputPath(contract, options.output);
  rmSync(output, { force: true });

  const targetRecords = [];
  for (const target of inventory.targets) {
    targetRecords.push(await captureTarget(target, contract, sourceCommit, credentials));
  }
  if (targetRecords.length !== inventory.targets.length) {
    fail("partial expected-target capture is forbidden");
  }

  writeAtomic(output, {
    format: "page_builder_provider_health_deployment_identity_v1",
    status: "deployment_identity_verified_health_evaluation_pending",
    captured_at: new Date().toISOString(),
    deployment: {
      deployment_id: inventory.deployment_id,
      deployment_image_digest: deploymentImageDigest,
      source_commit: sourceCommit,
      inventory_complete: true,
      expected_target_count: inventory.targets.length,
      verified_target_count: targetRecords.length,
      origin_to_repo_digest_binding: "maintainer_reviewed_external_fact",
      cryptographic_origin_to_repo_digest_binding: false,
    },
    expected_targets: targetRecords,
    source_files: sourceHashes(contract),
    credentials: {
      environment_names: credentials.environment_names,
      values_persisted: false,
    },
    privacy: {
      raw_metrics_urls_persisted: false,
      raw_metrics_responses_persisted: false,
      credential_values_persisted: false,
      tenant_page_revision_or_correlation_ids_persisted: false,
    },
    prometheus_backend_query_executed: false,
    provider_health_snapshot_evaluated: false,
    pages_provider_health_observed: false,
    pages_reference_consumer_gate_accepted: false,
    forum_wave_accepted: false,
    ffa_promoted: false,
    fba_promoted: false,
  });
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
