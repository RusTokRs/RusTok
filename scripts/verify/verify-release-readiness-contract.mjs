#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

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

function read(relativePath) {
  const file = path.join(repoRoot, relativePath);
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
  return source;
}

function count(source, marker) {
  return source.split(marker).length - 1;
}

const checklistPath = "docs/release/RELEASE_READINESS_CHECKLIST.md";
const checklist = requireMarkers(checklistPath, [
  "# Release Readiness Checklist",
  "## 1. Release identity",
  "## 2. Repository and registry preflight",
  "## 3. Required verification before tagging",
  "## 4. Tag workflow evidence",
  "## 5. Deployment and post-release smoke",
  "## 6. Rollback decision",
  "## 7. Failed-release recovery",
  "## 8. Evidence record",
  "repository immutable releases are enabled",
  "cryptographically verified **annotated** tag",
  "release-infra-approved",
  "Deploy the image by immutable digest",
  "Do not move, recreate or overwrite a published SemVer tag",
  "Failure after the version image tag is pushed but before GitHub Release publication",
  "Stop blind reruns",
  "Do not overwrite the version tag with a different digest",
  "Treat the release as immutable",
  "A checkbox without a durable run, artifact or operator record is not release evidence.",
]);

for (const asset of [
  "rustok-server-VERSION-linux-x86_64.tar.gz",
  "rustok-server-VERSION.spdx.json",
  "container-image.json",
  "release-manifest.json",
  "SHA256SUMS",
]) {
  if (count(checklist, asset) !== 1) {
    failures.push(`${checklistPath}: expected exactly one canonical asset entry for ${asset}`);
  }
}

requireMarkers("docs/verification/PLATFORM_HARDENING_STATUS_2026-07-18.md", [
  "HARD-101 — CSP enforcement",
  "HARD-204 — API compatibility",
  "HARD-205 — migration compatibility",
  "HARD-206 — release source contract",
  "Release ancestry fetch residual",
  "Failed-release recovery must be rehearsed",
  "It does **not** mean tests, CI, a browser smoke, a migration run or a production release succeeded.",
]);

requireMarkers(".github/workflows/release.yml", [
  "verify-release-collisions.mjs",
  'test "${#assets[@]}" -eq 5',
  "sha256sum --check SHA256SUMS",
  "subject-checksums: release-artifacts/SHA256SUMS",
  "sbom-path: release-artifacts/${{ needs.build.outputs.sbom_name }}",
  "--verify-tag",
]);

requireMarkers(".github/workflows/release-infrastructure.yml", [
  "Verify release readiness with base-owned policy",
  "base/scripts/verify/verify-release-readiness-contract.mjs",
  "Verify explicitly approved release readiness policy",
  "head/scripts/verify/verify-release-readiness-contract.mjs",
  '--root "$GITHUB_WORKSPACE/head"',
]);
requireMarkers(".github/workflows/hardening-gates.yml", [
  "Verify release readiness documentation",
  "verify-release-readiness-contract.mjs",
]);

if (failures.length > 0) {
  console.error(`Release readiness contract verification failed for ${repoRoot}:`);
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  `✔ release readiness, exact asset evidence, digest rollback and failed-release recovery are bound in ${repoRoot}`,
);
