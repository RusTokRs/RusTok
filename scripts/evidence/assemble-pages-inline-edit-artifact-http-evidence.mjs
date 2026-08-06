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
    if (["--build-a", "--build-b", "--docker", "--http", "--anonymous", "--output"].includes(argument)) {
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
  return {
    path: path.relative(repoRoot, location).startsWith("..")
      ? location
      : path.relative(repoRoot, location),
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

function validateArtifact(record, label) {
  requireObject(record, label);
  requirePositiveInteger(record.bytes, `${label}.bytes`);
  requireDigest(record.sha256, `${label}.sha256`);
  if (typeof record.path !== "string" || !record.path) fail(`${label}.path is required`);
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
  requireObject(build.toolchain, `${expectedProfile}.toolchain`);
  for (const name of ["node", "cargo", "rustc", "trunk", "wasm_bindgen"]) {
    if (typeof build.toolchain[name] !== "string" || !build.toolchain[name]) {
      fail(`${expectedProfile}.toolchain.${name} is required`);
    }
  }
  requireObject(build.source_sha256, `${expectedProfile}.source_sha256`);
  for (const sourcePath of contract.required_source_files) {
    requireDigest(build.source_sha256[sourcePath], `${expectedProfile}.source_sha256.${sourcePath}`);
  }
  requireObject(build.artifacts, `${expectedProfile}.artifacts`);
  for (const id of contract.build_snapshots.required_artifacts) {
    validateArtifact(build.artifacts[id], `${expectedProfile}.artifacts.${id}`);
  }
  if (!Array.isArray(build.admin_dist_manifest) || build.admin_dist_manifest.length === 0) {
    fail(`${expectedProfile}.admin_dist_manifest must be non-empty`);
  }
  for (const [index, artifact] of build.admin_dist_manifest.entries()) {
    validateArtifact(artifact, `${expectedProfile}.admin_dist_manifest[${index}]`);
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
  if (document.platform !== contract.docker_capture.required_platform) {
    fail("Docker platform drifted");
  }
  if (document.runtime?.user !== contract.docker_capture.required_user) {
    fail("Docker runtime user drifted");
  }
  if (!document.runtime?.entrypoint?.includes(contract.docker_capture.required_entrypoint)) {
    fail("Docker entrypoint drifted");
  }
  if (document.oci?.revision !== head) fail("Docker OCI revision does not match source commit");
  if (!Array.isArray(document.repo_digests) || document.repo_digests.length === 0) {
    fail("Docker immutable repo digest is missing");
  }
  for (const digest of document.repo_digests) {
    if (!/@sha256:[0-9a-f]{64}$/u.test(digest)) fail(`invalid Docker RepoDigest ${digest}`);
  }
  if (document.privacy?.docker_inspect_document_persisted !== false) {
    fail("Docker capture persisted the raw inspect document");
  }
  return document;
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
  if (!Array.isArray(document.assets) || document.assets.length !== contract.http_capture.asset_paths.length) {
    fail("HTTP asset evidence count drifted");
  }
  const buildMapping = {
    authoring_bootstrap: "authoring_bootstrap",
    authoring_module: "authoring_module",
    authoring_wasm: "authoring_wasm",
  };
  for (const specification of contract.http_capture.asset_paths) {
    const asset = document.assets.find((candidate) => candidate.id === specification.id);
    if (!asset || asset.path !== specification.path) fail(`HTTP asset ${specification.id} is missing`);
    if (asset.initial?.status !== 200) fail(`${specification.id} initial status must be 200`);
    if (asset.initial?.headers?.["content-type"] !== specification.content_type) {
      fail(`${specification.id} content type drifted`);
    }
    if (asset.initial?.headers?.["cache-control"] !== contract.http_capture.asset_cache_control) {
      fail(`${specification.id} cache control drifted`);
    }
    if (
      asset.initial?.headers?.["cross-origin-resource-policy"] !==
      contract.http_capture.asset_cross_origin_resource_policy
    ) {
      fail(`${specification.id} CORP drifted`);
    }
    requireDigest(asset.initial?.body_sha256, `${specification.id}.initial.body_sha256`);
    requirePositiveInteger(asset.initial?.body_bytes, `${specification.id}.initial.body_bytes`);
    if (asset.initial.body_sha256 !== build.artifacts[buildMapping[specification.id]].sha256) {
      fail(`${specification.id} HTTP body does not match the built artifact`);
    }
    for (const [label, response] of [
      ["exact", asset.exact_if_none_match],
      ["weak", asset.weak_if_none_match],
    ]) {
      if (response?.status !== 304 || response?.body_bytes !== 0) {
        fail(`${specification.id} ${label} conditional response must be empty 304`);
      }
    }
  }

  if (!Array.isArray(document.authoring) || document.authoring.length !== contract.http_capture.authoring_scenarios.length) {
    fail("HTTP authoring scenario count drifted");
  }
  for (const specification of contract.http_capture.authoring_scenarios) {
    const scenario = document.authoring.find((candidate) => candidate.id === specification.id);
    if (!scenario || scenario.response?.status !== specification.expected_status) {
      fail(`${specification.id} authoring status drifted`);
    }
    if (scenario.response?.headers?.["cache-control"] !== contract.http_capture.authoring_route_cache_control) {
      fail(`${specification.id} authoring cache control drifted`);
    }
    if (scenario.response?.headers?.["x-robots-tag"] !== contract.http_capture.authoring_route_robots) {
      fail(`${specification.id} authoring robots policy drifted`);
    }
    requireDigest(scenario.response?.body_sha256, `${specification.id}.response.body_sha256`);
    if (scenario.credential_values_persisted !== false) {
      fail(`${specification.id} persisted credential values`);
    }
    if (specification.id === "direct_user") {
      for (const value of Object.values(scenario.required_markers_present ?? {})) {
        if (value !== true) fail("direct_user required HTML marker is missing");
      }
      for (const value of Object.values(scenario.forbidden_markers_present ?? {})) {
        if (value !== false) fail("direct_user forbidden HTML marker was observed");
      }
    }
  }
  if (
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
  if (!Array.isArray(document.artifacts) || document.artifacts.length === 0) {
    fail("anonymous artifact evidence has no inspected artifacts");
  }
  for (const artifact of document.artifacts) {
    validateArtifact(artifact, `anonymous artifact ${artifact.path ?? "<unknown>"}`);
    if (!Array.isArray(artifact.forbidden_markers_found) || artifact.forbidden_markers_found.length !== 0) {
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
  if (!options[required]) fail(`--${required.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)} is required`);
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
  },
  http: {
    origin: http.target.origin,
    locale: http.target.locale,
    assets: http.assets,
    authoring: http.authoring,
  },
  anonymous_artifact: {
    profile: anonymous.profile,
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
