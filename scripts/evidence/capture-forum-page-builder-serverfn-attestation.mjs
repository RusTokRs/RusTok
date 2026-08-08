#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
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
  "crates/rustok-forum/contracts/evidence/forum-page-builder-serverfn-deployment-attestation-contract.json",
);
const MAX_BODY_BYTES = 1024 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const REQUEST_TIMEOUT_MS = 30_000;

function fail(message) {
  throw new Error(`Forum Page Builder server-function attestation failed: ${message}`);
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
        "usage: capture-forum-page-builder-serverfn-attestation.mjs " +
          "--base-url URL --deployment-image-digest REPO@sha256:DIGEST " +
          "[--source-commit SHA] [--output FILE]",
      );
      process.exit(0);
    }
    if (["--base-url", "--deployment-image-digest", "--source-commit", "--output"].includes(argument)) {
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

function requireBaseUrl(value) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail("--base-url must be an absolute HTTP(S) origin");
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

function boundedSecretEnvironment(name, maximumLength) {
  const value = process.env[name];
  if (value === undefined || value === "") return null;
  if (value.length > maximumLength || /[\r\n\u0000]/u.test(value)) {
    fail(`${name} is outside the bounded credential input`);
  }
  return value;
}

function commonHeaders(environmentName) {
  const value = process.env[environmentName];
  if (!value) return { headers: {}, environment_names: [] };
  if (value.length > 16_384 || /[\u0000]/u.test(value)) {
    fail(`${environmentName} is outside the bounded common-header input`);
  }
  let parsed;
  try {
    parsed = JSON.parse(value);
  } catch (error) {
    fail(`${environmentName} must contain a JSON object: ${error.message}`);
  }
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    fail(`${environmentName} must contain a JSON object`);
  }
  const headers = {};
  for (const [headerName, headerValue] of Object.entries(parsed)) {
    const normalized = headerName.toLowerCase();
    if (!/^[a-z0-9!#$%&'*+.^_`|~-]+$/u.test(normalized)) {
      fail(`${environmentName} contains an invalid header name`);
    }
    if (["authorization", "cookie", "set-cookie", "content-length"].includes(normalized)) {
      fail(`${environmentName} must not carry credential or framing headers`);
    }
    if (Object.hasOwn(headers, normalized)) {
      fail(`${environmentName} contains a duplicate case-insensitive header ${normalized}`);
    }
    if (
      typeof headerValue !== "string" ||
      headerValue.length > 4096 ||
      /[\r\n\u0000]/u.test(headerValue)
    ) {
      fail(`${environmentName}.${headerName} is outside the bounded header input`);
    }
    headers[normalized] = headerValue;
  }
  return { headers, environment_names: [environmentName] };
}

function credentialHeaders(contract, profile, shared) {
  if (profile === "none") {
    return { headers: { ...shared.headers }, environment_names: [...shared.environment_names] };
  }
  const prefix = profile === "authorized" ? "authorized" : "no_read";
  const authorizationName = contract.environment[`${prefix}_authorization`];
  const cookieName = contract.environment[`${prefix}_cookie`];
  const authorization = boundedSecretEnvironment(authorizationName, 8192);
  const cookie = boundedSecretEnvironment(cookieName, 16_384);
  if (!authorization && !cookie) {
    fail(`${profile} requires ${authorizationName} or ${cookieName}`);
  }
  const headers = { ...shared.headers };
  const environmentNames = [...shared.environment_names];
  if (authorization) {
    headers.authorization = authorization;
    environmentNames.push(authorizationName);
  }
  if (cookie) {
    headers.cookie = cookie;
    environmentNames.push(cookieName);
  }
  return {
    headers,
    environment_names: [...new Set(environmentNames)].sort(),
  };
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

function selectedHeaders(headers) {
  const output = {};
  for (const name of ["cache-control", "content-type", "x-content-type-options"]) {
    const value = headers.get(name);
    if (value !== null) output[name] = value;
  }
  return output;
}

async function requestScenario(baseUrl, endpoint, challenge, scenario, credentials) {
  const body = new URLSearchParams({ challenge }).toString();
  let response;
  try {
    response = await fetch(new URL(endpoint, baseUrl), {
      method: "POST",
      headers: {
        ...credentials.headers,
        "content-type": "application/x-www-form-urlencoded",
      },
      body,
      redirect: "manual",
      signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
    });
  } catch (error) {
    fail(`${scenario.id} request failed: ${error.message}`);
  }
  const responseBody = Buffer.from(await response.arrayBuffer());
  if (responseBody.byteLength > MAX_BODY_BYTES) {
    fail(`${scenario.id} response exceeds ${MAX_BODY_BYTES} bytes`);
  }
  if (scenario.expected_status !== undefined && response.status !== scenario.expected_status) {
    fail(`${scenario.id} expected ${scenario.expected_status}, got ${response.status}`);
  }
  if (scenario.success_forbidden && response.status === 200) {
    fail(`${scenario.id} unexpectedly reached a successful attestation response`);
  }
  return {
    response,
    responseBody,
    record: {
      id: scenario.id,
      credential_environment_names: credentials.environment_names,
      credential_values_persisted: false,
      status: response.status,
      headers: selectedHeaders(response.headers),
      body_bytes: responseBody.byteLength,
      body_sha256: sha256(responseBody),
      raw_body_persisted: false,
    },
  };
}

function requireAuthorizedBody(contract, body, challenge, sourceCommit) {
  const text = body.toString("utf8");
  for (const marker of contract.authorized_response.required_markers) {
    if (!text.includes(marker)) fail(`authorized response is missing marker ${marker}`);
  }
  if (!text.includes(challenge)) {
    fail("authorized response did not round-trip the current challenge");
  }
  if (!text.includes(sourceCommit)) {
    fail("authorized live transport did not report the exact checkout source commit");
  }
}

function outputPath(contract, requested) {
  const candidate = requested ?? contract.output.default_path;
  if (typeof candidate !== "string" || candidate.length === 0 || candidate.length > 16_384) {
    fail("output path is invalid");
  }
  const absolute = path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("attestation output must remain inside repository target/");
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
  if (!options.baseUrl) fail("--base-url is required");
  if (!options.deploymentImageDigest) fail("--deployment-image-digest is required");

  const contract = JSON.parse(readFileSync(contractPath, "utf8"));
  if (contract.status !== "source_ready_maintainer_execution_pending") {
    fail("attestation contract must not claim execution before capture starts");
  }
  const baseUrl = requireBaseUrl(options.baseUrl);
  const deploymentImageDigest = requireDeploymentImageDigest(options.deploymentImageDigest);
  const head = currentCommit();
  const sourceCommit = options.sourceCommit
    ? requireCommit(options.sourceCommit, "--source-commit")
    : head;
  if (sourceCommit !== head) {
    fail(`source commit ${sourceCommit} does not match git HEAD ${head}`);
  }

  const output = outputPath(contract, options.output);
  rmSync(output, { force: true });
  const sources = sourceHashes(contract);
  const challenge = `forum-attest-${randomUUID()}`;
  const shared = commonHeaders(contract.environment.common_headers_json);
  const scenarioRecords = [];

  for (const scenario of contract.scenarios) {
    const credentials = credentialHeaders(contract, scenario.credential_profile, shared);
    const captured = await requestScenario(
      baseUrl,
      contract.endpoint,
      challenge,
      scenario,
      credentials,
    );
    if (scenario.id === "authorized") {
      requireAuthorizedBody(contract, captured.responseBody, challenge, sourceCommit);
    }
    scenarioRecords.push(captured.record);
  }

  writeAtomic(output, {
    format: contract.output.format,
    status: contract.output.status,
    source_commit: sourceCommit,
    live_server_source_commit: sourceCommit,
    captured_at: new Date().toISOString(),
    target: {
      origin_bytes: Buffer.byteLength(baseUrl),
      origin_sha256: sha256(baseUrl),
      raw_origin_persisted: false,
      deployment_image_digest: deploymentImageDigest,
      origin_to_repo_digest_binding: "maintainer_reviewed_external_fact",
      cryptographic_origin_to_repo_digest_binding: false,
    },
    challenge: {
      bytes: Buffer.byteLength(challenge),
      sha256: sha256(challenge),
      raw_value_persisted: false,
    },
    source_files: sources,
    scenarios: scenarioRecords,
    privacy: {
      credential_environment_names_only: true,
      credential_values_persisted: false,
      common_header_values_persisted: false,
      raw_response_bodies_persisted: false,
      tenant_or_actor_identifiers_persisted: false,
      forum_content_persisted: false,
    },
    browser_execution_not_claimed: true,
    runtime_authorization_execution_not_claimed: true,
    provider_slo_health_not_claimed: true,
    observed_page_builder_wave_pending: true,
  });
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
