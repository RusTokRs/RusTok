#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const DEFAULT_APPROVAL_LABEL = "api-contract-infra-approved";
const PROTECTED_PATHS = [
  ".github/workflows/api-compatibility.yml",
  "apps/server/src/bin/export_api_contracts.rs",
  "scripts/verify/verify-api-compatibility.mjs",
  "scripts/verify/verify-api-compatibility-self-test.mjs",
  "scripts/verify/verify-api-compatibility-exceptions.mjs",
  "scripts/verify/verify-api-compatibility-exceptions-local.mjs",
  "scripts/verify/verify-api-compatibility-exception-approval.mjs",
  "scripts/verify/verify-api-compatibility-exception-approval-self-test.mjs",
  "scripts/verify/verify-api-compatibility-contract.mjs",
  "scripts/verify/verify-api-compatibility-infrastructure-approval.mjs",
];

function parseArguments(argv) {
  const options = {
    approvalLabel: DEFAULT_APPROVAL_LABEL,
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
    if (["--base-dir", "--head-dir", "--labels-json", "--approval-label"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a value`);
      options[argument.slice(2).replace(/-([a-z])/g, (_, character) => character.toUpperCase())] = value;
      index += 1;
      continue;
    }
    if (argument === "--explicitly-approved") {
      const value = argv[index + 1];
      if (!value || !/^(?:true|false)$/i.test(value)) {
        throw new Error(`${argument} requires true or false`);
      }
      options.explicitlyApproved = value.toLowerCase() === "true";
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }

  return options;
}

function parseLabels(labelsJson) {
  const labels = JSON.parse(labelsJson);
  if (!Array.isArray(labels) || labels.some((label) => typeof label !== "string")) {
    throw new Error("--labels-json must contain a JSON array of label names");
  }
  return new Set(labels);
}

function fileState(root, relativePath) {
  const absolutePath = path.join(root, relativePath);
  if (!fs.existsSync(absolutePath)) return { exists: false, content: null };
  return {
    exists: true,
    content: fs.readFileSync(absolutePath, "utf8").replaceAll("\r\n", "\n"),
  };
}

function changedProtectedPaths(baseRoot, headRoot, protectedPaths = PROTECTED_PATHS) {
  return protectedPaths.filter((relativePath) => {
    const base = fileState(baseRoot, relativePath);
    const head = fileState(headRoot, relativePath);
    return base.exists !== head.exists || base.content !== head.content;
  });
}

function approvalDecision({ changedPaths, labels, approvalLabel, explicitlyApproved }) {
  if (changedPaths.length === 0) return { approvalRequired: false, approved: true };
  return {
    approvalRequired: true,
    approved: explicitlyApproved || labels.has(approvalLabel),
  };
}

function runSelfTest() {
  const unchanged = approvalDecision({
    changedPaths: [],
    labels: new Set(),
    approvalLabel: DEFAULT_APPROVAL_LABEL,
    explicitlyApproved: false,
  });
  assert.deepEqual(unchanged, { approvalRequired: false, approved: true });

  const unapproved = approvalDecision({
    changedPaths: ["apps/server/src/bin/export_api_contracts.rs"],
    labels: new Set(),
    approvalLabel: DEFAULT_APPROVAL_LABEL,
    explicitlyApproved: false,
  });
  assert.deepEqual(unapproved, { approvalRequired: true, approved: false });

  const labelApproved = approvalDecision({
    changedPaths: ["scripts/verify/verify-api-compatibility.mjs"],
    labels: new Set([DEFAULT_APPROVAL_LABEL]),
    approvalLabel: DEFAULT_APPROVAL_LABEL,
    explicitlyApproved: false,
  });
  assert.deepEqual(labelApproved, { approvalRequired: true, approved: true });

  const dispatchApproved = approvalDecision({
    changedPaths: [".github/workflows/api-compatibility.yml"],
    labels: new Set(),
    approvalLabel: DEFAULT_APPROVAL_LABEL,
    explicitlyApproved: true,
  });
  assert.deepEqual(dispatchApproved, { approvalRequired: true, approved: true });

  console.log("✔ API compatibility infrastructure approval self-test passed");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  if (!options.baseDir || !options.headDir) {
    throw new Error(
      "usage: verify-api-compatibility-infrastructure-approval.mjs --base-dir DIR --head-dir DIR [--labels-json JSON] [--explicitly-approved true|false]",
    );
  }

  const baseRoot = path.resolve(options.baseDir);
  const headRoot = path.resolve(options.headDir);
  const changedPaths = changedProtectedPaths(baseRoot, headRoot);
  const decision = approvalDecision({
    changedPaths,
    labels: parseLabels(options.labelsJson),
    approvalLabel: options.approvalLabel,
    explicitlyApproved: options.explicitlyApproved,
  });

  if (!decision.approvalRequired) {
    console.log("✔ API compatibility infrastructure is unchanged");
    return;
  }
  if (!decision.approved) {
    console.error(
      `API compatibility infrastructure changed without ${options.approvalLabel} approval:`,
    );
    changedPaths.forEach((relativePath) => console.error(`✗ ${relativePath}`));
    process.exit(Math.min(changedPaths.length, 255));
  }

  console.log(
    `✔ API compatibility infrastructure change is explicitly approved (${options.approvalLabel}): ${changedPaths.join(", ")}`,
  );
}

try {
  main();
} catch (error) {
  console.error(`API compatibility infrastructure approval verification failed: ${error.message}`);
  process.exit(1);
}
