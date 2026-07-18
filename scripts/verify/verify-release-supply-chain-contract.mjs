#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const ACTIONS = Object.freeze({
  checkout: "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0",
  setupNode: "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38",
  uploadArtifact: "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
  downloadArtifact: "actions/download-artifact@634f93cb2916e3fdff6788551b99b062d0335ce0",
  attest: "actions/attest@f7c74d28b9d84cb8768d0b8ca14a4bac6ef463e6",
});

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--root") {
      const value = argv[index + 1];
      if (!value) throw new Error("--root requires a value");
      options.root = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argv[index]}`);
  }
  return options;
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const options = parseArguments(process.argv.slice(2));
const repoRoot = path.resolve(options.root || path.resolve(scriptDir, "../.."));
const failures = [];

function absolute(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const file = absolute(relativePath);
  if (!fs.existsSync(file)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  const stats = fs.lstatSync(file);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${relativePath}: must be a regular non-symlink file`);
    return "";
  }
  return fs.readFileSync(file, "utf8");
}

function requireMarkers(relativePath, markers) {
  const source = read(relativePath);
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${relativePath}: missing marker ${marker}`);
  }
}

function forbidMarkers(relativePath, markers) {
  const source = read(relativePath);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relativePath}: forbidden marker ${marker}`);
  }
}

function count(source, marker) {
  return source.split(marker).length - 1;
}

function requireCount(relativePath, marker, expected) {
  const actual = count(read(relativePath), marker);
  if (actual !== expected) {
    failures.push(`${relativePath}: expected ${expected} occurrence(s) of ${marker}, found ${actual}`);
  }
}

function actionReferences(relativePath) {
  return [...read(relativePath).matchAll(/^\s*uses:\s*([^\s#]+)(?:\s+#.*)?\s*$/gm)].map(
    (match) => match[1],
  );
}

function requirePinnedGithubActions(relativePath, expectedCounts) {
  const references = actionReferences(relativePath);
  for (const reference of references) {
    if (!reference.startsWith("actions/")) {
      failures.push(`${relativePath}: non-GitHub action is forbidden: ${reference}`);
      continue;
    }
    if (!/^actions\/[A-Za-z0-9_.-]+@[0-9a-f]{40}$/.test(reference)) {
      failures.push(`${relativePath}: action must be pinned to a full lowercase commit SHA: ${reference}`);
    }
  }
  const allowed = new Set(Object.values(ACTIONS));
  for (const reference of references) {
    if (!allowed.has(reference)) failures.push(`${relativePath}: unapproved action reference ${reference}`);
  }
  for (const [reference, expected] of expectedCounts) {
    const actual = references.filter((candidate) => candidate === reference).length;
    if (actual !== expected) {
      failures.push(`${relativePath}: expected ${expected} use(s) of ${reference}, found ${actual}`);
    }
  }
}

const releaseWorkflow = ".github/workflows/release.yml";
requireMarkers(releaseWorkflow, [
  "name: Release",
  'tags:\n      - "v*.*.*"',
  "cancel-in-progress: false",
  "refs/tags/${GITHUB_REF_NAME}^{tag}",
  ".verification.verified",
  "git merge-base --is-ancestor",
  "repos/${GITHUB_REPOSITORY}/immutable-releases",
  "verify-release-collisions.mjs",
  "--github-release",
  "verify-release-contract.mjs",
  "cargo build --locked --release -p rustok-server --bin rustok-server",
  "rustup toolchain install 1.96.0 --profile minimal --no-self-update",
  "package-server.sh",
  "cargo metadata --locked --format-version 1",
  "generate-spdx-sbom.mjs",
  "Release archive is not reproducible",
  "--container-tag",
  "--platform linux/amd64",
  "--provenance=mode=max",
  "--sbom=true",
  "containerimage.digest",
  "subject-checksums: release-artifacts/SHA256SUMS",
  "sbom-path: release-artifacts/${{ needs.build.outputs.sbom_name }}",
  "finalize-release-artifacts.mjs",
  "sha256sum --check SHA256SUMS",
  "extract-release-notes.mjs",
  'test "${#assets[@]}" -eq 5',
  "release create",
  "--verify-tag",
  "packages: write",
  "id-token: write",
  "attestations: write",
  "artifact-metadata: write",
]);
requireCount(releaseWorkflow, "persist-credentials: false", 6);
requireCount(releaseWorkflow, "verify-release-collisions.mjs", 3);
requireCount(releaseWorkflow, "cargo build --locked --release -p rustok-server --bin rustok-server", 2);
forbidMarkers(releaseWorkflow, [
  "workflow_dispatch:",
  "pull_request:",
  "pull_request_target:",
  "continue-on-error:",
  "runs-on: ubuntu-latest",
  "gh release upload",
  "gh release view",
  "docker buildx imagetools inspect",
  "--clobber",
  "--provenance=false",
  "--sbom=false",
  "actions/checkout@v",
  "actions/setup-node@v",
  "actions/upload-artifact@v",
  "actions/download-artifact@v",
  "actions/attest@v",
]);
requirePinnedGithubActions(
  releaseWorkflow,
  new Map([
    [ACTIONS.checkout, 6],
    [ACTIONS.setupNode, 5],
    [ACTIONS.uploadArtifact, 3],
    [ACTIONS.downloadArtifact, 4],
    [ACTIONS.attest, 3],
  ]),
);

const infrastructureWorkflow = ".github/workflows/release-infrastructure.yml";
requireMarkers(infrastructureWorkflow, [
  "name: Release Infrastructure",
  "pull_request_target:",
  "workflow_dispatch:",
  "allow_infrastructure_changes:",
  "permissions:\n  contents: read",
  "Require release-infra-approved for release infrastructure changes",
  "base/scripts/verify/verify-release-infrastructure-approval.mjs",
  "steps.policy.outputs.changed == 'false'",
  "base/scripts/verify/verify-release-supply-chain-contract.mjs",
  "steps.policy.outputs.changed == 'true'",
  "head/scripts/verify/verify-release-tooling-self-test.mjs",
  "--root \"$GITHUB_WORKSPACE/head\"",
  "persist-credentials: false",
]);
requireCount(infrastructureWorkflow, "persist-credentials: false", 2);
forbidMarkers(infrastructureWorkflow, [
  "permissions:\n  contents: write",
  "packages: write",
  "id-token: write",
  "attestations: write",
  "artifact-metadata: write",
  "secrets:",
  "continue-on-error:",
  "runs-on: ubuntu-latest",
  "actions/checkout@v",
  "actions/setup-node@v",
]);
requirePinnedGithubActions(
  infrastructureWorkflow,
  new Map([
    [ACTIONS.checkout, 2],
    [ACTIONS.setupNode, 1],
  ]),
);

const hardeningWorkflow = ".github/workflows/hardening-gates.yml";
requireMarkers(hardeningWorkflow, [
  "Verify release tooling fixtures",
  "verify-release-tooling-self-test.mjs",
  "Verify release infrastructure approval fixtures",
  "verify-release-infra-self-test.mjs",
  "Verify release supply-chain gate structure",
  "verify-release-supply-chain-contract.mjs",
]);
forbidMarkers(hardeningWorkflow, ["actions/checkout@v", "actions/setup-node@v"]);
requirePinnedGithubActions(
  hardeningWorkflow,
  new Map([
    [ACTIONS.checkout, 1],
    [ACTIONS.setupNode, 1],
  ]),
);

requireMarkers("scripts/release/verify-release-contract.mjs", [
  "CANONICAL_SEMVER",
  "workspace version",
  "Cargo.lock must contain exactly one ${packageName} package",
  "contains an unreplaced placeholder",
  "duplicate [${section}] section",
  "new Date(timestamp).toISOString().slice(0, 10) !== releaseDate",
  "function runSelfTest",
]);
requireMarkers("scripts/release/verify-release-collisions.mjs", [
  "status === 404",
  "unexpected HTTP ${status}",
  "AbortSignal.timeout(15_000)",
  "api.github.com/repos/${repository}/releases/tags",
  "https://ghcr.io/v2/${repositoryPath}/manifests",
  "GHCR authentication challenge is incomplete",
  "function runSelfTest",
]);
requireMarkers("scripts/release/generate-spdx-sbom.mjs", [
  'spdxVersion: "SPDX-2.3"',
  'dataLicense: "CC0-1.0"',
  "function reachablePackageIds",
  "dependency graph is missing resolve node",
  "DEPENDS_ON",
  "rustok-admin",
  '"licenseListVersion" in parsed.creationInfo',
  "function runSelfTest",
]);
requireMarkers("scripts/release/finalize-release-artifacts.mjs", [
  "function requireExactNames",
  "--image-metadata must be <directory>/container-image.json",
  "release source asset set",
  "checksummed release asset set",
  "rustok-server-${options.version}-linux-x86_64.tar.gz",
  "rustok-server-${options.version}.spdx.json",
  "unexpected.txt",
  "function runSelfTest",
]);
requireMarkers("scripts/release/package-server.sh", [
  "set -euo pipefail",
  "--sort=name",
  '--mtime="@$epoch"',
  "--numeric-owner",
  "gzip -n -9",
  "config directory must not contain symlinks",
]);
forbidMarkers("scripts/release/package-server.sh", ["eval ", "tar -czf", "|| true"]);
requireMarkers("scripts/release/extract-release-notes.mjs", [
  "RELEASE_HEADING",
  "exactly one release heading",
  "function runSelfTest",
]);
requireMarkers("scripts/verify/verify-release-tooling-self-test.mjs", [
  "verify-release-contract.mjs",
  "verify-release-collisions.mjs",
  "generate-spdx-sbom.mjs",
  "finalize-release-artifacts.mjs",
  "extract-release-notes.mjs",
  "package-server.sh",
]);
requireMarkers("scripts/verify/verify-release-infrastructure-approval.mjs", [
  'const APPROVAL_LABEL = "release-infra-approved"',
  ".github/workflows/release.yml",
  ".github/workflows/release-infrastructure.yml",
  ".github/workflows/hardening-gates.yml",
  "scripts/verify/verify-all.sh",
  "verify-release-collisions.mjs",
  "function changedProtectedPaths",
  "function approvalDecision",
]);

requireMarkers("apps/server/Dockerfile.release", [
  "FROM debian:${DEBIAN_VERSION}-slim",
  "org.opencontainers.image.revision",
  "--uid 10001 --gid 10001",
  "COPY --chown=10001:10001 rustok-server",
  "release image config must not contain symlinks",
  "USER 10001:10001",
  'ENTRYPOINT ["/app/rustok-server"]',
]);
forbidMarkers("apps/server/Dockerfile.release", [
  "postgresql-client",
  "curl",
  "USER root",
  "|| true",
  "cargo build",
]);
requireMarkers("apps/server/Dockerfile", [
  "cargo build --locked --release -p rustok-server --bin rustok-server",
  "--uid 10001 --gid 10001",
  "USER 10001:10001",
]);
requireMarkers(".dockerignore", [".git", ".github", "**/target", "**/node_modules", ".env.*"]);
requireMarkers("scripts/verify/verify-all.sh", [
  "verify-release-tooling-self-test.mjs:Release Tooling Fixtures",
  "verify-release-infra-self-test.mjs:Release Infrastructure Approval Fixtures",
  "verify-release-supply-chain-contract.mjs:Release Supply-chain Gate Structure",
]);

if (failures.length > 0) {
  console.error(`Release supply-chain contract verification failed for ${repoRoot}:`);
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  `✔ release workflows use commit-pinned GitHub actions and preserve signed tags, immutable releases, reproducible exact assets, scoped SPDX, attestations and base-owned approval in ${repoRoot}`,
);
