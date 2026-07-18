#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--root") {
      const value = argv[index + 1];
      if (!value) throw new Error("--root requires a value");
      options.root = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
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

function requireFile(relativePath) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${relativePath}: required file is missing`);
    return false;
  }
  return true;
}

function read(relativePath) {
  return fs.readFileSync(absolute(relativePath), "utf8");
}

function requireMarkers(relativePath, markers) {
  if (!requireFile(relativePath)) return;
  const source = read(relativePath);
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${relativePath}: missing marker ${marker}`);
  }
}

function forbidMarkers(relativePath, markers) {
  if (!requireFile(relativePath)) return;
  const source = read(relativePath);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relativePath}: forbidden marker ${marker}`);
  }
}

function requireOccurrenceCount(relativePath, marker, expected) {
  if (!requireFile(relativePath)) return;
  const actual = read(relativePath).split(marker).length - 1;
  if (actual !== expected) {
    failures.push(`${relativePath}: expected ${expected} occurrence(s) of ${marker}, found ${actual}`);
  }
}

function actionReferences(relativePath) {
  if (!requireFile(relativePath)) return [];
  return [...read(relativePath).matchAll(/^\s*uses:\s*([^\s#]+)\s*$/gm)].map(
    (match) => match[1],
  );
}

function requireGithubOwnedActions(relativePath, requiredActions) {
  const references = actionReferences(relativePath);
  for (const reference of references) {
    if (!reference.startsWith("actions/")) {
      failures.push(`${relativePath}: workflow may only use GitHub-owned actions, found ${reference}`);
    }
    if (!/@v\d+$/.test(reference)) {
      failures.push(`${relativePath}: action reference must use an explicit major release tag, found ${reference}`);
    }
  }
  for (const reference of requiredActions) {
    if (!references.includes(reference)) {
      failures.push(`${relativePath}: missing required action ${reference}`);
    }
  }
}

const releaseWorkflow = ".github/workflows/release.yml";
requireMarkers(releaseWorkflow, [
  "name: Release",
  'tags:\n      - "v*.*.*"',
  "cancel-in-progress: false",
  "Validate signed release tag",
  "refs/tags/${GITHUB_REF_NAME}^{tag}",
  ".verification.verified",
  "git merge-base --is-ancestor",
  "refs/remotes/origin/main",
  "Require repository release immutability",
  "repos/${GITHUB_REPOSITORY}/immutable-releases",
  "--jq '.enabled'",
  "Repository immutable releases must be enabled before publishing a tag",
  "verify-release-contract.mjs",
  "--workspace Cargo.toml",
  "--lock Cargo.lock",
  "--changelog CHANGELOG.md",
  "Build deterministic Linux artifact",
  "Rebuild and compare archive digest",
  "Release archive is not reproducible",
  "rustup toolchain install 1.96.0 --profile minimal --no-self-update",
  "cargo build --locked --release -p rustok-server --bin rustok-server",
  "package-server.sh",
  "cargo metadata --locked --format-version 1",
  "generate-spdx-sbom.mjs",
  "Publish attested GHCR image",
  "docker login ghcr.io",
  "--platform linux/amd64",
  "--provenance=mode=max",
  "--sbom=true",
  "--push",
  "containerimage.digest",
  "actions/attest@v4",
  "push-to-registry: true",
  "subject-checksums: release-artifacts/SHA256SUMS",
  "sbom-path: release-artifacts/${{ needs.build.outputs.sbom_name }}",
  "finalize-release-artifacts.mjs",
  "sha256sum --check SHA256SUMS",
  "extract-release-notes.mjs",
  "Release $RELEASE_TAG already exists; refusing to mutate published assets",
  "gh release view",
  "release create",
  "--verify-tag",
  "contents: write",
  "packages: write",
  "id-token: write",
  "attestations: write",
  "artifact-metadata: write",
]);
requireOccurrenceCount(
  releaseWorkflow,
  "cargo build --locked --release -p rustok-server --bin rustok-server",
  2,
);
requireOccurrenceCount(
  releaseWorkflow,
  "rustup toolchain install 1.96.0 --profile minimal --no-self-update",
  2,
);
requireOccurrenceCount(releaseWorkflow, "actions/attest@v4", 3);
requireOccurrenceCount(releaseWorkflow, "persist-credentials: false", 6);
requireOccurrenceCount(releaseWorkflow, "repos/${GITHUB_REPOSITORY}/immutable-releases", 1);
forbidMarkers(releaseWorkflow, [
  "workflow_dispatch:",
  "pull_request:",
  "pull_request_target:",
  "continue-on-error:",
  "runs-on: ubuntu-latest",
  "cargo build --release -p rustok-server",
  "--provenance=false",
  "--sbom=false",
  "packages: read",
  "contents: write\n      packages: write\n      id-token: read",
  "gh release upload",
  "--clobber",
  "immutable-releases\" >/dev/null || true",
]);
requireGithubOwnedActions(
  releaseWorkflow,
  new Set([
    "actions/checkout@v7",
    "actions/setup-node@v6",
    "actions/upload-artifact@v7",
    "actions/download-artifact@v5",
    "actions/attest@v4",
  ]),
);

const infrastructureWorkflow = ".github/workflows/release-infrastructure.yml";
requireMarkers(infrastructureWorkflow, [
  "name: Release Infrastructure",
  "pull_request_target:",
  "workflow_dispatch:",
  "allow_infrastructure_changes:",
  "permissions:\n  contents: read",
  "BASE_REPOSITORY:",
  "HEAD_REPOSITORY:",
  "BASE_REF:",
  "HEAD_REF:",
  "Release supply-chain policy",
  "runs-on: ubuntu-24.04",
  "timeout-minutes: 10",
  "Checkout base policy source",
  "Checkout proposed release source",
  "persist-credentials: false",
  "Verify base approval policy fixtures",
  "base/scripts/verify/verify-release-infrastructure-approval.mjs",
  "--self-test",
  "Require approval for release infrastructure changes",
  "release-infra-approved",
  "--base-dir \"$GITHUB_WORKSPACE/base\"",
  "--head-dir \"$GITHUB_WORKSPACE/head\"",
  "--github-output \"$GITHUB_OUTPUT\"",
  "Verify unchanged head with base-owned release policy",
  "steps.policy.outputs.changed == 'false'",
  "base/scripts/verify/verify-release-supply-chain-contract.mjs",
  "--root \"$GITHUB_WORKSPACE/head\"",
  "Verify explicitly approved tooling fixtures",
  "steps.policy.outputs.changed == 'true'",
  "head/scripts/verify/verify-release-tooling-self-test.mjs",
  "Verify explicitly approved head policy",
  "head/scripts/verify/verify-release-supply-chain-contract.mjs",
]);
requireOccurrenceCount(infrastructureWorkflow, "persist-credentials: false", 2);
requireOccurrenceCount(infrastructureWorkflow, "steps.policy.outputs.changed == 'true'", 2);
forbidMarkers(infrastructureWorkflow, [
  "permissions:\n  contents: write",
  "packages: write",
  "id-token: write",
  "attestations: write",
  "artifact-metadata: write",
  "secrets:",
  "continue-on-error:",
  "runs-on: ubuntu-latest",
  "pull_request:\n",
]);
requireGithubOwnedActions(
  infrastructureWorkflow,
  new Set(["actions/checkout@v7", "actions/setup-node@v6"]),
);

requireMarkers("scripts/release/verify-release-contract.mjs", [
  "CANONICAL_SEMVER",
  "release tag must start with v",
  "workspace version",
  "Cargo.lock must contain exactly one ${packageName} package",
  "must contain exactly one release heading",
  "contains an unreplaced placeholder",
  "duplicate [${section}] section",
  "--github-output",
  "function runSelfTest",
]);
requireMarkers("scripts/release/generate-spdx-sbom.mjs", [
  'spdxVersion: "SPDX-2.3"',
  'dataLicense: "CC0-1.0"',
  "cargo metadata resolve.nodes must be present",
  "DEPENDS_ON",
  "--created-epoch",
  "Workspace package built from the release commit",
  "function runSelfTest",
]);
requireMarkers("scripts/release/finalize-release-artifacts.mjs", [
  "release artifact directory must contain regular files only",
  "must not be a symlink",
  "container metadata digest must be sha256",
  "release-manifest.json",
  "SHA256SUMS",
  "size_bytes",
  "function runSelfTest",
]);
requireMarkers("scripts/release/extract-release-notes.mjs", [
  "RELEASE_HEADING",
  "exactly one release heading",
  "Released ${heading[2]}",
  "function runSelfTest",
]);
requireMarkers("scripts/release/package-server.sh", [
  "set -euo pipefail",
  "--sort=name",
  '--mtime="@$epoch"',
  "--owner=0",
  "--group=0",
  "--numeric-owner",
  "gzip -n -9",
  "config directory must not contain symlinks",
  "deterministic package self-test produced different digests",
]);
forbidMarkers("scripts/release/package-server.sh", ["eval ", "tar -czf", "|| true"]);

requireMarkers("apps/server/Dockerfile.release", [
  "FROM debian:${DEBIAN_VERSION}-slim",
  "org.opencontainers.image.source",
  "org.opencontainers.image.version",
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
  "COPY . .",
  "cargo build --locked --release -p rustok-server --bin rustok-server",
  "--uid 10001 --gid 10001",
  "USER 10001:10001",
  'ENTRYPOINT ["/app/rustok-server"]',
]);
forbidMarkers("apps/server/Dockerfile", [
  "postgresql-client",
  "cargo build --release --bin rustok-server",
]);

requireMarkers(".dockerignore", [
  ".git",
  ".github",
  "**/target",
  "**/node_modules",
  ".env.*",
  "!.env.example",
]);
requireMarkers("scripts/verify/verify-release-tooling-self-test.mjs", [
  "verify-release-contract.mjs",
  "generate-spdx-sbom.mjs",
  "finalize-release-artifacts.mjs",
  "extract-release-notes.mjs",
  "package-server.sh",
  "--self-test",
]);
requireMarkers("scripts/verify/verify-release-infrastructure-approval.mjs", [
  'const APPROVAL_LABEL = "release-infra-approved"',
  ".github/workflows/release.yml",
  ".github/workflows/release-infrastructure.yml",
  "apps/server/Dockerfile.release",
  "generate-spdx-sbom.mjs",
  "function changedProtectedPaths",
  "function approvalDecision",
  "--github-output",
  "function runSelfTest",
]);
requireMarkers("scripts/verify/verify-release-infra-self-test.mjs", [
  "verify-release-infrastructure-approval.mjs",
  '"--self-test"',
]);

requireMarkers(".github/workflows/hardening-gates.yml", [
  "Verify release tooling fixtures",
  "verify-release-tooling-self-test.mjs",
  "Verify release infrastructure approval fixtures",
  "verify-release-infra-self-test.mjs",
  "Verify release supply-chain gate structure",
  "verify-release-supply-chain-contract.mjs",
]);
requireMarkers("scripts/verify/verify-all.sh", [
  "release-tooling-self-test  Verify deterministic release tooling fixtures",
  "release-infra-self-test  Verify release-infrastructure approval policy fixtures",
  "release-supply-chain-contract  Verify signed, reproducible and attested release structure",
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
  `✔ signed SemVer tags, immutable releases, reproducible archives, SPDX SBOM, checksums, attestations, GHCR publication and base-owned release policy are structurally bound in ${repoRoot}`,
);
