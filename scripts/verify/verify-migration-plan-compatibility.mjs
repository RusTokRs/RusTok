#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

function parseArguments(argv) {
  const options = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      options.selfTest = true;
      continue;
    }
    if (["--base", "--head"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a file path`);
      options[argument.slice(2)] = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  return options;
}

function readPlan(file) {
  const resolved = path.resolve(file);
  const plan = JSON.parse(fs.readFileSync(resolved, "utf8"));
  const failures = [];

  if (plan.schema_version !== 1) failures.push("schema_version must be 1");
  if (!Array.isArray(plan.migrations)) {
    failures.push("migrations must be an array");
  } else {
    const names = new Set();
    for (const [index, name] of plan.migrations.entries()) {
      if (typeof name !== "string" || name.trim() === "") {
        failures.push(`migrations[${index}] must be a non-empty string`);
        continue;
      }
      if (names.has(name)) failures.push(`migration name is duplicated: ${name}`);
      names.add(name);
    }
  }

  if (failures.length > 0) {
    throw new Error(`${resolved}:\n${failures.map((failure) => `- ${failure}`).join("\n")}`);
  }
  return plan.migrations;
}

function comparePlans(base, head) {
  const failures = [];
  if (head.length < base.length) {
    failures.push(
      `head migration plan has ${head.length} entries but base has ${base.length}`,
    );
  }

  const sharedLength = Math.min(base.length, head.length);
  for (let index = 0; index < sharedLength; index += 1) {
    if (base[index] !== head[index]) {
      failures.push(
        `migration ${index + 1} changed from ${JSON.stringify(base[index])} to ${JSON.stringify(head[index])}`,
      );
    }
  }

  return failures;
}

function runSelfTest() {
  assert.deepEqual(comparePlans(["a", "b"], ["a", "b", "c"]), []);
  assert.deepEqual(comparePlans(["a", "b"], ["a"]), [
    "head migration plan has 1 entries but base has 2",
  ]);
  assert.deepEqual(comparePlans(["a", "b"], ["a", "renamed"]), [
    'migration 2 changed from "b" to "renamed"',
  ]);
  assert.deepEqual(comparePlans(["a", "b"], ["b", "a"]), [
    'migration 1 changed from "a" to "b"',
    'migration 2 changed from "b" to "a"',
  ]);
  console.log("✔ migration plan compatibility self-test passed");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  if (!options.base || !options.head) {
    throw new Error(
      "usage: verify-migration-plan-compatibility.mjs --base FILE --head FILE",
    );
  }

  const base = readPlan(options.base);
  const head = readPlan(options.head);
  const failures = comparePlans(base, head);
  if (failures.length > 0) {
    console.error("Migration plan compatibility verification failed:");
    failures.forEach((failure) => console.error(`✗ ${failure}`));
    process.exit(Math.min(failures.length, 255));
  }

  console.log(
    `✔ migration history is append-only (${base.length} base entries, ${head.length - base.length} appended)`,
  );
}

try {
  main();
} catch (error) {
  console.error(`Migration plan compatibility verification failed: ${error.message}`);
  process.exit(1);
}
