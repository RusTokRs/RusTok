#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const DEFAULT_APPROVAL_LABEL = "api-breaking-approved";

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
    if (["--base-file", "--head-file", "--labels-json", "--approval-label"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a value`);
      options[argument.slice(2).replace(/-([a-z])/g, (_, character) => character.toUpperCase())] = value;
      index += 1;
      continue;
    }
    if (argument === "--explicitly-approved") {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires true or false`);
      if (!/^(?:true|false)$/i.test(value)) {
        throw new Error(`${argument} must be true or false`);
      }
      options.explicitlyApproved = value.toLowerCase() === "true";
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }

  return options;
}

function stableJson(value) {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function parseLabels(labelsJson) {
  const labels = JSON.parse(labelsJson);
  if (!Array.isArray(labels) || labels.some((label) => typeof label !== "string")) {
    throw new Error("--labels-json must contain a JSON array of label names");
  }
  return new Set(labels);
}

function approvalDecision({ baseRegister, headRegister, labels, approvalLabel, explicitlyApproved }) {
  const changed = stableJson(baseRegister) !== stableJson(headRegister);
  if (!changed) return { changed: false, approved: true };
  const approved = explicitlyApproved || labels.has(approvalLabel);
  return { changed: true, approved };
}

function runSelfTest() {
  const base = { schema_version: 1, policy: { review_by: "2026-08-15" }, exceptions: [] };
  const sameWithDifferentKeyOrder = {
    exceptions: [],
    policy: { review_by: "2026-08-15" },
    schema_version: 1,
  };
  const changed = {
    ...base,
    exceptions: [
      {
        id: "graphql:field-removed:Query.legacy",
        owner: "API maintainers",
        reason: "Versioned migration",
        expires_on: "2026-08-01",
      },
    ],
  };

  assert.deepEqual(
    approvalDecision({
      baseRegister: base,
      headRegister: sameWithDifferentKeyOrder,
      labels: new Set(),
      approvalLabel: DEFAULT_APPROVAL_LABEL,
      explicitlyApproved: false,
    }),
    { changed: false, approved: true },
  );
  assert.deepEqual(
    approvalDecision({
      baseRegister: base,
      headRegister: changed,
      labels: new Set(),
      approvalLabel: DEFAULT_APPROVAL_LABEL,
      explicitlyApproved: false,
    }),
    { changed: true, approved: false },
  );
  assert.deepEqual(
    approvalDecision({
      baseRegister: base,
      headRegister: changed,
      labels: new Set([DEFAULT_APPROVAL_LABEL]),
      approvalLabel: DEFAULT_APPROVAL_LABEL,
      explicitlyApproved: false,
    }),
    { changed: true, approved: true },
  );
  assert.deepEqual(
    approvalDecision({
      baseRegister: base,
      headRegister: changed,
      labels: new Set(),
      approvalLabel: DEFAULT_APPROVAL_LABEL,
      explicitlyApproved: true,
    }),
    { changed: true, approved: true },
  );

  console.log("✔ API compatibility exception approval self-test passed");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  if (!options.baseFile || !options.headFile) {
    throw new Error(
      "usage: verify-api-compatibility-exception-approval.mjs --base-file FILE --head-file FILE [--labels-json JSON] [--explicitly-approved true|false]",
    );
  }

  const baseFile = path.resolve(options.baseFile);
  const headFile = path.resolve(options.headFile);
  const baseRegister = JSON.parse(fs.readFileSync(baseFile, "utf8"));
  const headRegister = JSON.parse(fs.readFileSync(headFile, "utf8"));
  const labels = parseLabels(options.labelsJson);
  const decision = approvalDecision({
    baseRegister,
    headRegister,
    labels,
    approvalLabel: options.approvalLabel,
    explicitlyApproved: options.explicitlyApproved,
  });

  if (!decision.changed) {
    console.log("✔ API compatibility exception register is unchanged");
    return;
  }
  if (!decision.approved) {
    console.error(
      `API compatibility exception register changed without ${options.approvalLabel} approval`,
    );
    process.exit(1);
  }

  console.log(
    `✔ API compatibility exception register change is explicitly approved (${options.approvalLabel})`,
  );
}

try {
  main();
} catch (error) {
  console.error(`API compatibility exception approval verification failed: ${error.message}`);
  process.exit(1);
}
