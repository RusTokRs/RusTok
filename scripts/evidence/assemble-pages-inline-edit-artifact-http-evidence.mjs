#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-pages/contracts/evidence/pages-inline-edit-artifact-http-execution-contract.json";
const contract = JSON.parse(readFileSync(path.join(repoRoot, contractPath), "utf8"));
const emptySha256 = createHash("sha256").update(Buffer.alloc(0)).digest("hex");

function fail(message) {
  throw new Error(`Pages inline edit artifact/HTTP evidence assembly failed: ${message}`);
}

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: assemble-pages-inline-edit-artifact-http-evidence.mjs " +
          "--build-a FILE --build-b FILE --docker FILE --http FILE " +
          "--anonymous FILE --output FILE",
      );
      process.exit(0);
    }
    if (
      [
        "--build-a",
        "--build-b",
        "--docker",
        "--http",
        "--anonymous",
        "--output",
      ].includes(argument)
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

function absolute(value) {
  return path.isAbsolute(value) ? path.resolve(value) : path.resolve(repoRoot, value);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function readJson(file, label) {
  const location = absolute(file);
  let bytes;
  try {
    bytes = readFileSync(location);
  } catch (error) {
    fail(`${label} is unavailable: ${error.message}`);
  }
  let document;
  try {
    document = JSON.parse(bytes.toString("utf8"));
  } catch (error) {
    fail(`${label} is not valid JSON: ${error.message}`);
  }
  const relative = path.relative(repoRoot, location);
  return {
    path: relative.startsWith("..") ? location : relative,
    bytes,
    sha256: sha256(bytes),
    document,
  };
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

function requireObject(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
}

function requireString(value, label, maximumLength = 4096) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > maximumLength ||
    /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/u.test(value)
  ) {
    fail(`${label} must be a bounded non-empty string`);
  }
  return value;
}

function requireDigest(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    fail(`${label} must be a lowercase SHA-256 digest`);
  }
  return value;
}

function requirePositiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0) {
    fail(`${label} must be a positive safe integer`);
  }
  return value;
}

function requireNonNegativeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) {
    fail(`${label} must be a non-negative safe integer`);
  }
  return value;
}

function requireIsoTimestamp(value, label) {
  requireString(value, label, 128);
  if (!Number.isFinite(Date.parse(value))) fail(`${label} must be an ISO timestamp`);
  return value;
}

function stable(value) {
  if (Array.isArray(value)) return value.map(stable);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, item]) => [key, stable(item)]),
    );
  }
  return value;
}

function same(left, right) {
  return JSON.stringify(stable(left)) === JSON.stringify(stable(right));
}

function requireExactKeys(object, expected, label) {
  requireObject(object, label);
  const actual = Object.keys(object).sort();
  const wanted = [...expected].sort();
  if (!same(actual, wanted)) {
    fail(`${label} keys drifted: expected ${wanted.join(", ")}, got ${actual.join(", ")}`);
  }
}

function validateArtifact(record, label) {
  requireObject(record, label);
  requireString(record.path, `${label}.path`, 16_384);
  requirePositiveInteger(record.bytes, `${label}.bytes`);
  requireDigest(record.sha256, `${label}.sha256`);
  return record;
}

function validateBuild(input, expectedProfile, head) {
  const build = input.document;
  if (
    build.format !== contract.build_snapshots.format ||
    build.status !== "passed" ||
    build.profile !== expectedProfile ||
    build.source_commit !== head
  ) {
    fail(`${expectedProfile} identity, status, or source commit drifted`);
  }
  requireIsoTimestamp(build.captured_at, `${expectedProfile}.captured_at`);

  requireExactKeys(
    build.toolchain,
    ["node", "cargo", "rustc", "trunk", "wasm_bindgen"],
    `${expectedProfile}.toolchain`,
  );
  for (const [name, value] of Object.entries(build.toolchain)) {
    requireString(value, `${expectedProfile}.toolchain.${name}`, 2048);
  }

  requireObject(build.build_command_log, `${expectedProfile}.build_command_log`);
  requirePositiveInteger(
    build.build_command_log.bytes,
    `${expectedProfile}.build_command_log.bytes`,
  );
  requireDigest(
    build.build_command_log.sha256,
    `${expectedProfile}.build_command_log.sha256`,
  );
  if (build.build_command_log.raw_output_persisted !== false) {
    fail(`${expectedProfile} persisted raw build command output`);
  }

  requireObject(build.source_sha256, `${expectedProfile}.source_sha256`);
  requireExactKeys(
    build.source_sha256,
    contract.required_source_files,
    `${expectedProfile}.source_sha256`,
  );
  for (const sourcePath of contract.required_source_files) {
    requireDigest(
      build.source_sha256[sourcePath],
      `${expectedProfile}.source_sha256.${sourcePath}`,
    );
  }

  requireExactKeys(
    build.artifacts,
    contract.build_snapshots.required_artifacts,
    `${expectedProfile}.artifacts`,
  );
  for (const id of contract.build_snapshots.required_artifacts) {
    validateArtifact(build.artifacts[id], `${expectedProfile}.artifacts.${id}`);
  }
  if (build.artifacts.server_binary.executable !== true) {
    fail(`${expectedProfile}.artifacts.server_binary must be executable`);
  }

  if (!Array.isArray(build.admin_dist_manifest) || build.admin_dist_manifest.length === 0) {
    fail(`${expectedProfile}.admin_dist_manifest must be non-empty`);
  }
  const manifestPaths = [];
  for (const [index, artifact] of build.admin_dist_manifest.entries()) {
    validateArtifact(artifact, `${expectedProfile}.admin_dist_manifest[${index}]`);
    manifestPaths.push(artifact.path);
  }
  const sortedPaths = [...manifestPaths].sort((left, right) => left.localeCompare(right));
  if (!same(manifestPaths, sortedPaths) || new Set(manifestPaths).size !== manifestPaths.length) {
    fail(`${expectedProfile}.admin_dist_manifest must be sorted and unique`);
  }

  if (
    build.privacy?.raw_command_log_persisted !== false ||
    build.privacy?.credentials_persisted !== false ||
    build.privacy?.grants_or_proofs_persisted !== false
  ) {
    fail(`${expectedProfile} privacy boundary drifted`);
  }
  return build;
}

function compareBuilds(buildA, buildB) {
  if (!same(buildA.toolchain, buildB.toolchain)) {
    fail("build toolchain versions do not match");
  }
  if (!same(buildA.source_sha256, buildB.source_sha256)) {
    fail("build source hashes do not match");
  }
  for (const id of contract.build_snapshots.required_artifacts) {
    const left = buildA.artifacts[id];
    const right = buildB.artifacts[id];
    if (left.sha256 !== right.sha256 || left.bytes !== right.bytes) {
      fail(`${id} is not reproducible between build-a and build-b`);
    }
  }
  if (!same(buildA.admin_dist_manifest, buildB.admin_dist_manifest)) {
    fail("embedded admin dist manifest is not reproducible between builds");
  }
}

function validateDocker(input, head) {
  const document = input.document;
  if (
    document.format !== contract.docker_capture.format ||
    document.status !== "passed" ||
    document.source_commit !== head
  ) {
    fail("Docker evidence identity, status, or source commit drifted");
  }
  requireIsoTimestamp(document.captured_at, "Docker captured_at");
  requireString(document.requested_image, "Docker requested_image", 512);
  if (!/^sha256:[0-9a-f]{64}$/u.test(document.image_id ?? "")) {
    fail("Docker image_id must be a canonical SHA-256 digest");
  }
  requirePositiveInteger(document.size_bytes, "Docker size_bytes");
  if (document.platform !== contract.docker_capture.required_platform) {
    fail("Docker platform drifted");
  }
  if (document.runtime?.user !== contract.docker_capture.required_user) {
    fail("Docker runtime user drifted");
  }
  if (
    !Array.isArray(document.runtime?.entrypoint) ||
    !document.runtime.entrypoint.includes(contract.docker_capture.required_entrypoint)
  ) {
    fail("Docker entrypoint drifted");
  }
  if (document.oci?.revision !== head) {
    fail("Docker OCI revision does not match source commit");
  }
  if (!Array.isArray(document.repo_digests) || document.repo_digests.length === 0) {
    fail("Docker immutable repo digest is missing");
  }
  const sortedDigests = [...document.repo_digests].sort();
  if (!same(document.repo_digests, sortedDigests) || new Set(sortedDigests).size !== sortedDigests.length) {
    fail("Docker RepoDigests must be sorted and unique");
  }
  for (const digest of document.repo_digests) {
    if (!/@sha256:[0-9a-f]{64}$/u.test(digest)) {
      fail(`invalid Docker RepoDigest ${digest}`);
    }
  }
  requireObject(document.inspect_output, "Docker inspect_output");
  requirePositiveInteger(document.inspect_output.bytes, "Docker inspect_output.bytes");
  requireDigest(document.inspect_output.sha256, "Docker inspect_output.sha256");
  if (document.inspect_output.raw_document_persisted !== false) {
    fail("Docker capture persisted the raw inspect document");
  }
  if (
    document.privacy?.docker_inspect_document_persisted !== false ||
    document.privacy?.environment_values_persisted !== false ||
    document.privacy?.credentials_persisted !== false
  ) {
    fail("Docker privacy boundary drifted");
  }
  return document;
}

function validateResponseRecord(record, label, allowEmpty) {
  requireObject(record, label);
  requireNonNegativeInteger(record.body_bytes, `${label}.body_bytes`);
  requireDigest(record.body_sha256, `${label}.body_sha256`);
  if (!allowEmpty && record.body_bytes === 0) fail(`${label} body must be non-empty`);
  if (record.body_bytes === 0 && record.body_sha256 !== emptySha256) {
    fail(`${label} empty body SHA-256 drifted`);
  }
  if (record.raw_body_persisted !== false) fail(`${label} persisted a raw response body`);
  requireObject(record.headers, `${label}.headers`);
}

function validateConditionalResponse(assetId, label, response, etag) {
  validateResponseRecord(response, `${assetId}.${label}`, true);
  if (response.status !== 304 || response.body_bytes !== 0) {
    fail(`${assetId} ${label} conditional response must be empty 304`);
  }
  if (response.headers.etag !== etag) fail(`${assetId} ${label} ETag drifted`);
  if (response.headers["cache-control"] !== contract.http_capture.asset_cache_control) {
    fail(`${assetId} ${label} cache control drifted`);
  }
  if (
    response.headers["cross-origin-resource-policy"] !==
    contract.http_capture.asset_cross_origin_resource_policy
  ) {
    fail(`${assetId} ${label} CORP drifted`);
  }
}

function validateCredentialEnvironmentNames(scenario, specification) {
  if (!Array.isArray(scenario.credential_environment_names)) {
    fail(`${specification.id} credential_environment_names must be an array`);
  }
  const names = scenario.credential_environment_names;
  if (new Set(names).size !== names.length || !same(names, [...names].sort())) {
    fail(`${specification.id} credential environment names must be sorted and unique`);
  }
  const allowed = new Set([
    "RUSTOK_PAGES_INLINE_EDIT_EVIDENCE_COMMON_HEADERS_JSON",
    specification.authorization_env,
    specification.cookie_env,
  ].filter(Boolean));
  for (const name of names) {
    if (!allowed.has(name)) fail(`${specification.id} contains unexpected credential environment ${name}`);
  }
  if (
    specification.id !== "anonymous" &&
    !names.includes(specification.authorization_env) &&
    !names.includes(specification.cookie_env)
  ) {
    fail(`${specification.id} has no scenario credential environment name`);
  }
}

function validateHttp(input, head, build) {
  const document = input.document;
  if (
    document.format !== contract.http_capture.format ||
    document.status !== "passed" ||
    document.source_commit !== head
  ) {
    fail("HTTP evidence identity, status, or source commit drifted");
  }
  requireIsoTimestamp(document.captured_at, "HTTP captured_at");
  requireString(document.target?.origin, "HTTP target origin", 2048);
  requireString(document.target?.locale, "HTTP target locale", 64);
  if (document.target?.page_id_shape !== "uuid") fail("HTTP target page id shape drifted");

  if (
    !Array.isArray(document.assets) ||
    document.assets.length !== contract.http_capture.asset_paths.length
  ) {
    fail("HTTP asset evidence count drifted");
  }
  const assetIds = document.assets.map(({ id }) => id);
  const expectedAssetIds = contract.http_capture.asset_paths.map(({ id }) => id);
  if (!same([...assetIds].sort(), [...expectedAssetIds].sort())) {
    fail("HTTP asset identities drifted");
  }
  const buildMapping = {
    authoring_bootstrap: "authoring_bootstrap",
    authoring_module: "authoring_module",
    authoring_wasm: "authoring_wasm",
  };
  for (const specification of contract.http_capture.asset_paths) {
    const asset = document.assets.find((candidate) => candidate.id === specification.id);
    if (!asset || asset.path !== specification.path) {
      fail(`HTTP asset ${specification.id} is missing or has the wrong path`);
    }
    validateResponseRecord(asset.initial, `${specification.id}.initial`, false);
    if (asset.initial.status !== 200) fail(`${specification.id} initial status must be 200`);
    if (asset.initial.headers["content-type"] !== specification.content_type) {
      fail(`${specification.id} content type drifted`);
    }
    if (asset.initial.headers["cache-control"] !== contract.http_capture.asset_cache_control) {
      fail(`${specification.id} cache control drifted`);
    }
    if (
      asset.initial.headers["cross-origin-resource-policy"] !==
      contract.http_capture.asset_cross_origin_resource_policy
    ) {
      fail(`${specification.id} CORP drifted`);
    }
    const etag = asset.initial.headers.etag;
    if (etag !== `"${asset.initial.body_sha256}"`) {
      fail(`${specification.id} strong body-bound ETag drifted`);
    }
    if (
      asset.initial.body_sha256 !== build.artifacts[buildMapping[specification.id]].sha256 ||
      asset.initial.body_bytes !== build.artifacts[buildMapping[specification.id]].bytes
    ) {
      fail(`${specification.id} HTTP body does not match the built artifact`);
    }
    validateConditionalResponse(
      specification.id,
      "exact_if_none_match",
      asset.exact_if_none_match,
      etag,
    );
    validateConditionalResponse(
      specification.id,
      "weak_if_none_match",
      asset.weak_if_none_match,
      etag,
    );
  }

  if (
    !Array.isArray(document.authoring) ||
    document.authoring.length !== contract.http_capture.authoring_scenarios.length
  ) {
    fail("HTTP authoring scenario count drifted");
  }
  const scenarioIds = document.authoring.map(({ id }) => id);
  const expectedScenarioIds = contract.http_capture.authoring_scenarios.map(({ id }) => id);
  if (!same([...scenarioIds].sort(), [...expectedScenarioIds].sort())) {
    fail("HTTP authoring scenario identities drifted");
  }
  for (const specification of contract.http_capture.authoring_scenarios) {
    const scenario = document.authoring.find((candidate) => candidate.id === specification.id);
    if (!scenario || scenario.expected_status !== specification.expected_status) {
      fail(`${specification.id} expected status declaration drifted`);
    }
    validateResponseRecord(scenario.response, `${specification.id}.response`, false);
    if (scenario.response.status !== specification.expected_status) {
      fail(`${specification.id} authoring status drifted`);
    }
    if (
      scenario.response.headers["cache-control"] !==
      contract.http_capture.authoring_route_cache_control
    ) {
      fail(`${specification.id} authoring cache control drifted`);
    }
    if (
      scenario.response.headers["x-robots-tag"] !==
      contract.http_capture.authoring_route_robots
    ) {
      fail(`${specification.id} authoring robots policy drifted`);
    }
    if (scenario.credential_values_persisted !== false) {
      fail(`${specification.id} persisted credential values`);
    }
    validateCredentialEnvironmentNames(scenario, specification);

    if (specification.id === "direct_user") {
      const requiredKeys = [
        ...contract.http_capture.direct_user_required_markers,
        "page_id",
        "locale",
      ];
      requireExactKeys(
        scenario.required_markers_present,
        requiredKeys,
        "direct_user.required_markers_present",
      );
      for (const key of requiredKeys) {
        if (scenario.required_markers_present[key] !== true) {
          fail(`direct_user required HTML marker is missing: ${key}`);
        }
      }
      requireExactKeys(
        scenario.forbidden_markers_present,
        contract.http_capture.direct_user_forbidden_markers,
        "direct_user.forbidden_markers_present",
      );
      for (const key of contract.http_capture.direct_user_forbidden_markers) {
        if (scenario.forbidden_markers_present[key] !== false) {
          fail(`direct_user forbidden HTML marker was observed: ${key}`);
        }
      }
    } else {
      requireExactKeys(
        scenario.required_markers_present,
        [],
        `${specification.id}.required_markers_present`,
      );
      requireExactKeys(
        scenario.forbidden_markers_present,
        [],
        `${specification.id}.forbidden_markers_present`,
      );
    }
  }
  if (
    document.privacy?.credential_environment_names_only !== true ||
    document.privacy?.credential_values_persisted !== false ||
    document.privacy?.raw_response_bodies_persisted !== false ||
    document.privacy?.grants_or_proofs_persisted !== false
  ) {
    fail("HTTP privacy boundary drifted");
  }
  return document;
}

function validateAnonymous(input, head) {
  const document = input.document;
  if (
    document.format !== contract.anonymous_artifact_input.format ||
    document.status !== contract.anonymous_artifact_input.status ||
    document.source_commit !== head
  ) {
    fail("anonymous artifact evidence identity, status, or source commit drifted");
  }
  requireIsoTimestamp(document.generated_at, "anonymous generated_at");
  if (document.graph_verifier?.status !== "passed") {
    fail("anonymous dependency graph verifier did not pass");
  }
  requireString(document.graph_verifier?.command, "anonymous graph command", 4096);
  if (
    document.artifact_contract?.explicit_artifact_paths_required !== true ||
    document.artifact_contract
      ?.byte_scan_is_combined_with_feature_resolved_dependency_graph !== true ||
    document.artifact_contract
      ?.absence_of_a_client_bundle_is_not_reported_as_a_passing_client_bundle !== true
  ) {
    fail("anonymous artifact contract drifted");
  }
  if (!Array.isArray(document.artifacts) || document.artifacts.length === 0) {
    fail("anonymous artifact evidence has no inspected artifacts");
  }
  for (const artifact of document.artifacts) {
    validateArtifact(artifact, `anonymous artifact ${artifact.path ?? "<unknown>"}`);
    if (
      !Array.isArray(artifact.forbidden_markers_found) ||
      artifact.forbidden_markers_found.length !== 0
    ) {
      fail(`anonymous artifact ${artifact.path ?? "<unknown>"} contains authoring markers`);
    }
  }
  if (!Array.isArray(document.findings) || document.findings.length !== 0) {
    fail("anonymous artifact evidence contains findings");
  }
  return document;
}

function writeAtomic(output, document) {
  const location = absolute(output);
  mkdirSync(path.dirname(location), { recursive: true });
  const temporary = `${location}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, location);
  return location;
}

const options = parseArguments(process.argv.slice(2));
for (const required of ["buildA", "buildB", "docker", "http", "anonymous", "output"]) {
  if (!options[required]) {
    fail(`--${required.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)} is required`);
  }
}

const head = currentCommit();
const inputs = {
  build_a: readJson(options.buildA, "build-a snapshot"),
  build_b: readJson(options.buildB, "build-b snapshot"),
  docker: readJson(options.docker, "Docker evidence"),
  http: readJson(options.http, "HTTP evidence"),
  anonymous: readJson(options.anonymous, "anonymous artifact evidence"),
};
const buildA = validateBuild(inputs.build_a, "build-a", head);
const buildB = validateBuild(inputs.build_b, "build-b", head);
compareBuilds(buildA, buildB);
const docker = validateDocker(inputs.docker, head);
const http = validateHttp(inputs.http, head, buildA);
const anonymous = validateAnonymous(inputs.anonymous, head);

const inputManifest = Object.fromEntries(
  Object.entries(inputs).map(([id, input]) => [
    id,
    {
      path: input.path,
      bytes: input.bytes.length,
      sha256: input.sha256,
    },
  ]),
);
const artifactManifest = Object.fromEntries(
  contract.build_snapshots.required_artifacts.map((id) => [id, buildA.artifacts[id]]),
);

const document = {
  format: contract.output.format,
  status: contract.output.status,
  source_commit: head,
  generated_at: new Date().toISOString(),
  input_manifest: inputManifest,
  reproducibility: {
    profiles: contract.build_snapshots.profiles,
    toolchain: buildA.toolchain,
    source_sha256: buildA.source_sha256,
    build_command_logs: {
      build_a: buildA.build_command_log,
      build_b: buildB.build_command_log,
    },
    critical_artifacts: artifactManifest,
    admin_dist_manifest: buildA.admin_dist_manifest,
    critical_hashes_match: true,
    admin_dist_manifest_matches: true,
  },
  docker: {
    image_id: docker.image_id,
    repo_digests: docker.repo_digests,
    size_bytes: docker.size_bytes,
    platform: docker.platform,
    runtime: docker.runtime,
    oci: docker.oci,
    inspect_output: docker.inspect_output,
  },
  http: {
    origin: http.target.origin,
    locale: http.target.locale,
    assets: http.assets,
    authoring: http.authoring,
  },
  anonymous_artifact: {
    profile: anonymous.profile,
    graph_verifier: anonymous.graph_verifier,
    artifacts: anonymous.artifacts,
    findings: anonymous.findings,
  },
  boundaries: {
    browser_edit_save_replay_expiry_executed: false,
    tenant_rollout_executed: false,
    ffa_promoted: false,
    fba_promoted: false,
    canonical_source_mutated: false,
  },
  privacy: {
    credential_values_persisted: false,
    grants_or_proofs_persisted: false,
    raw_response_bodies_persisted: false,
    raw_command_logs_persisted: false,
    docker_inspect_document_persisted: false,
  },
};

const output = writeAtomic(options.output, document);
console.log(
  `[assemble-pages-inline-edit-artifact-http-evidence] PASS source_commit=${head} output=${output}`,
);
