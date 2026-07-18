#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const APPROVAL_LABEL = "release-infra-approved";
const PROTECTED_PATHS = [
  ".github/workflows/release.yml",
  ".github/workflows/release-infrastructure.yml",
  ".github/workflows/hardening-gates.yml",
  ".dockerignore",
  "apps/admin/Trunk.toml",
  "apps/admin/index.html",
  "apps/admin/package.json",
  "apps/admin/package-lock.json",
  "apps/admin/scripts/tailwind-build.mjs",
  "apps/server/Dockerfile",
  "apps/server/Dockerfile.release",
  "docs/release/RELEASE_READINESS_CHECKLIST.md",
  "docs/verification/PLATFORM_HARDENING_STATUS_2026-07-18.md",
  "scripts/build/build-embedded-admin.sh",
  "scripts/release/verify-release-contract.mjs",
  "scripts/release/verify-release-collisions.mjs",
  "scripts/release/generate-spdx-sbom.mjs",
  "scripts/release/finalize-release-artifacts.mjs",
  "scripts/release/extract-release-notes.mjs",
  "scripts/release/package-server.sh",
  "scripts/verify/verify-all.sh",
  "scripts/verify/verify-release-tooling-self-test.mjs",
  "scripts/verify/verify-release-supply-chain-contract.mjs",
  "scripts/verify/verify-release-runtime-image-contract.mjs",
  "scripts/verify/verify-release-readiness-contract.mjs",
  "scripts/verify/verify-release-infrastructure-approval.mjs",
  "scripts/verify/verify-release-infra-self-test.mjs",
];

function parseArguments(argv) {
  const options = {
    labelsJson: "[]",
    explicitlyApproved: false,
    selfTest: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      options.selfTest = true;
      continue;
    }
    if (["--base-dir", "--head-dir", "--labels-json", "--github-output"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a value`);
      options[argument.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = value;
      index += 1;
      continue;
    }
    if (argument === "--explicitly-approved") {
      const value = argv[index + 1];
      if (!value || !/^(?:true|false)$/i.test(value)) {
        throw new Error("--explicitly-approved requires true or false");
      }
      options.explicitlyApproved = value.toLowerCase() === "true";
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  return options;
}

function parseLabels(value) {
  const labels = JSON.parse(value);
  if (!Array.isArray(labels) || labels.some((label) => typeof label !== "string")) {
    throw new Error("--labels-json must be a JSON array of label names");
  }
  return new Set(labels);
}

function fileState(root, relativePath) {
  const file = path.join(root, relativePath);
  if (!fs.existsSync(file)) return null;
  const stats = fs.lstatSync(file);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    throw new Error(`${relativePath} must be a regular non-symlink file`);
  }
  return fs.readFileSync(file, "utf8").replaceAll("\r\n", "\n");
}

function changedProtectedPaths(baseRoot, headRoot) {
  return PROTECTED_PATHS.filter(
    (relativePath) => fileState(baseRoot, relativePath) !== fileState(headRoot, relativePath),
  );
}

function approvalDecision(changedPaths, labels, explicitlyApproved) {
  if (changedPaths.length === 0) return { required: false, approved: true };
  return {
    required: true,
    approved: explicitlyApproved || labels.has(APPROVAL_LABEL),
  };
}

function writeGithubOutput(file, changedPaths) {
  if (!file) return;
  fs.appendFileSync(
    file,
    `changed=${changedPaths.length > 0 ? "true" : "false"}\nchanged_count=${changedPaths.length}\n`,
  );
}

function runSelfTest() {
  assert.deepEqual(approvalDecision([], new Set(), false), {
    required: false,
    approved: true,
  });
  assert.deepEqual(
    approvalDecision([PROTECTED_PATHS[0]], new Set(), false),
    { required: true, approved: false },
  );
  assert.deepEqual(
    approvalDecision([PROTECTED_PATHS[1]], new Set([APPROVAL_LABEL]), false),
    { required: true, approved: true },
  );
  assert.deepEqual(
    approvalDecision([PROTECTED_PATHS[2]], new Set(), true),
    { required: true, approved: true },
  );
  assert(PROTECTED_PATHS.includes(".github/workflows/release.yml"));
  assert(PROTECTED_PATHS.includes(".github/workflows/release-infrastructure.yml"));
  assert(PROTECTED_PATHS.includes(".github/workflows/hardening-gates.yml"));
  assert(PROTECTED_PATHS.includes("scripts/verify/verify-all.sh"));
  assert(PROTECTED_PATHS.includes("apps/admin/Trunk.toml"));
  assert(PROTECTED_PATHS.includes("apps/admin/package-lock.json"));
  assert(PROTECTED_PATHS.includes("apps/admin/scripts/tailwind-build.mjs"));
  assert(PROTECTED_PATHS.includes("scripts/build/build-embedded-admin.sh"));
  assert(PROTECTED_PATHS.includes("apps/server/Dockerfile.release"));
  assert(PROTECTED_PATHS.includes("docs/release/RELEASE_READINESS_CHECKLIST.md"));
  assert(PROTECTED_PATHS.includes("docs/verification/PLATFORM_HARDENING_STATUS_2026-07-18.md"));
  assert(PROTECTED_PATHS.includes("scripts/release/verify-release-collisions.mjs"));
  assert(PROTECTED_PATHS.includes("scripts/release/generate-spdx-sbom.mjs"));
  assert(PROTECTED_PATHS.includes("scripts/verify/verify-release-runtime-image-contract.mjs"));
  assert(PROTECTED_PATHS.includes("scripts/verify/verify-release-readiness-contract.mjs"));
  console.log("✔ release infrastructure approval self-test passed");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  if (!options.baseDir || !options.headDir) {
    throw new Error(
      "usage: verify-release-infrastructure-approval.mjs --base-dir DIR --head-dir DIR [--labels-json JSON] [--explicitly-approved true|false] [--github-output FILE]",
    );
  }
  const changedPaths = changedProtectedPaths(
    path.resolve(options.baseDir),
    path.resolve(options.headDir),
  );
  writeGithubOutput(options.githubOutput, changedPaths);
  const decision = approvalDecision(
    changedPaths,
    parseLabels(options.labelsJson),
    options.explicitlyApproved,
  );
  if (!decision.required) {
    console.log("✔ release supply-chain infrastructure is unchanged");
    return;
  }
  if (!decision.approved) {
    console.error(`release infrastructure changed without ${APPROVAL_LABEL} approval:`);
    changedPaths.forEach((relativePath) => console.error(`✗ ${relativePath}`));
    process.exit(Math.min(changedPaths.length, 255));
  }
  console.log(
    `✔ release infrastructure change is explicitly approved (${APPROVAL_LABEL}): ${changedPaths.join(", ")}`,
  );
}

try {
  main();
} catch (error) {
  console.error(`release infrastructure approval verification failed: ${error.message}`);
  process.exit(1);
}
