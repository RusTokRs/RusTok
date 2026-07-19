#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";

const APPROVAL_LABEL = "migration-infra-approved";
const PROTECTED_PATHS = [
  ".github/workflows/migration-compatibility.yml",
  ".github/workflows/migration-infrastructure-approval.yml",
  ".github/workflows/repository-ruleset-audit.yml",
  ".github/workflows/hardening-gates.yml",
  "docs/ci/repository-ruleset-contract.json",
  "docs/ci/repository-ruleset-contract.md",
  "crates/rustok-migrations/src/bin/export_migration_plan.rs",
  "crates/rustok-migrations/tests/postgres_zero_migration_smoke.rs",
  "crates/rustok-migrations/tests/support/mod.rs",
  "crates/rustok-migrations/tests/support/backfill_fixtures.rs",
  "scripts/verify/verify-migration-smoke.sh",
  "scripts/verify/verify-migration-plan-compatibility.mjs",
  "scripts/verify/verify-migration-plan-self-test.mjs",
  "scripts/verify/verify-migration-backfill-contracts.mjs",
  "scripts/verify/verify-migration-backfill-self-test.mjs",
  "scripts/verify/verify-migration-compatibility-contract.mjs",
  "scripts/verify/verify-migration-infrastructure-approval.mjs",
  "scripts/verify/verify-migration-infra-self-test.mjs",
  "scripts/verify/verify-repository-ruleset-contract.mjs",
  "scripts/verify/verify-repository-ruleset-self-test.mjs",
  "scripts/verify/verify-repository-ruleset-structure.mjs",
  "scripts/verify/verify-all.sh",
];

function parseArguments(argv) {
  const options = { labelsJson: "[]", explicitlyApproved: false, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      options.selfTest = true;
      continue;
    }
    if (["--base-dir", "--head-dir", "--labels-json"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a value`);
      options[argument.slice(2).replace(/-([a-z])/g, (_, character) => character.toUpperCase())] = value;
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
  const stats = fs.lstatSync(file, { throwIfNoEntry: false });
  if (!stats) return { kind: "missing" };
  if (stats.isSymbolicLink()) {
    return { kind: "symlink", target: fs.readlinkSync(file) };
  }
  if (!stats.isFile()) {
    return { kind: "non-regular" };
  }
  return {
    kind: "file",
    content: fs.readFileSync(file, "utf8").replaceAll("\r\n", "\n"),
  };
}

function isUnsafeFileState(state) {
  return state.kind === "symlink" || state.kind === "non-regular";
}

function unsafeProtectedPaths(root) {
  return PROTECTED_PATHS.filter((relativePath) => isUnsafeFileState(fileState(root, relativePath)));
}

function changedProtectedPaths(baseRoot, headRoot) {
  return PROTECTED_PATHS.filter(
    (relativePath) =>
      JSON.stringify(fileState(baseRoot, relativePath)) !==
      JSON.stringify(fileState(headRoot, relativePath)),
  );
}

function approvalDecision(changedPaths, labels, explicitlyApproved) {
  if (changedPaths.length === 0) return { required: false, approved: true };
  return {
    required: true,
    approved: explicitlyApproved || labels.has(APPROVAL_LABEL),
  };
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
  assert.equal(isUnsafeFileState({ kind: "file", content: "policy" }), false);
  assert.equal(isUnsafeFileState({ kind: "missing" }), false);
  assert.equal(isUnsafeFileState({ kind: "symlink", target: "../base/policy" }), true);
  assert.equal(isUnsafeFileState({ kind: "non-regular" }), true);

  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rustok-migration-approval-"));
  try {
    fs.writeFileSync(path.join(fixtureRoot, "target.txt"), "policy");
    fs.symlinkSync("target.txt", path.join(fixtureRoot, "link.txt"));
    fs.symlinkSync("missing.txt", path.join(fixtureRoot, "dangling.txt"));
    assert.deepEqual(fileState(fixtureRoot, "link.txt"), {
      kind: "symlink",
      target: "target.txt",
    });
    assert.deepEqual(fileState(fixtureRoot, "dangling.txt"), {
      kind: "symlink",
      target: "missing.txt",
    });
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
  console.log("✔ migration infrastructure approval self-test passed");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  if (!options.baseDir || !options.headDir) {
    throw new Error(
      "usage: verify-migration-infrastructure-approval.mjs --base-dir DIR --head-dir DIR [--labels-json JSON] [--explicitly-approved true|false]",
    );
  }

  const baseRoot = path.resolve(options.baseDir);
  const headRoot = path.resolve(options.headDir);
  const unsafeBasePaths = unsafeProtectedPaths(baseRoot);
  const unsafeHeadPaths = unsafeProtectedPaths(headRoot);
  if (unsafeBasePaths.length > 0 || unsafeHeadPaths.length > 0) {
    const details = [
      ...unsafeBasePaths.map((relativePath) => `base:${relativePath}`),
      ...unsafeHeadPaths.map((relativePath) => `head:${relativePath}`),
    ];
    throw new Error(
      `protected migration infrastructure must be regular files, refusing symlink or non-regular path(s): ${details.join(", ")}`,
    );
  }

  const changedPaths = changedProtectedPaths(baseRoot, headRoot);
  const decision = approvalDecision(
    changedPaths,
    parseLabels(options.labelsJson),
    options.explicitlyApproved,
  );

  if (!decision.required) {
    console.log("✔ migration compatibility infrastructure is unchanged");
    return;
  }
  if (!decision.approved) {
    console.error(
      `migration compatibility infrastructure changed without ${APPROVAL_LABEL} approval:`,
    );
    changedPaths.forEach((relativePath) => console.error(`✗ ${relativePath}`));
    process.exit(Math.min(changedPaths.length, 255));
  }

  console.log(
    `✔ migration compatibility infrastructure change is explicitly approved (${APPROVAL_LABEL}): ${changedPaths.join(", ")}`,
  );
}

try {
  main();
} catch (error) {
  console.error(`migration infrastructure approval verification failed: ${error.message}`);
  process.exit(1);
}
