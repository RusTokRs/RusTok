#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { isDeepStrictEqual } from "node:util";

const declarationFields = ["owner", "after", "safety", "use_transaction"];

function parseArguments(argv) {
  const options = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      options.selfTest = true;
      continue;
    }
    if (["--base", "--head", "--protected-prefix"].includes(argument)) {
      const value = argv[index + 1];
      const key = argument.slice(2);
      if (!value || value.startsWith("--")) throw new Error(`${argument} requires a file path`);
      if (Object.hasOwn(options, key)) throw new Error(`${argument} must be supplied once`);
      options[key] = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  return options;
}

function readPlan(file, requireDeclarations) {
  const resolved = path.resolve(file);
  const plan = JSON.parse(fs.readFileSync(resolved, "utf8"));
  const failures = validatePlan(plan, requireDeclarations);
  if (failures.length > 0) {
    throw new Error(`${resolved}:\n${failures.map((failure) => `- ${failure}`).join("\n")}`);
  }
  return plan;
}

function validatePlan(plan, requireDeclarations) {
  const failures = [];

  if (plan?.schema_version !== 1) failures.push("schema_version must be 1");
  if (!Array.isArray(plan?.migrations)) return [...failures, "migrations must be an array"];
  const names = new Set();
  for (const [index, name] of plan.migrations.entries()) {
    if (typeof name !== "string" || name.trim() === "") {
      failures.push(`migrations[${index}] must be a non-empty string`);
      continue;
    }
    if (names.has(name)) failures.push(`migration name is duplicated: ${name}`);
    names.add(name);
  }

  if (requireDeclarations && failures.length === 0) {
    return validateDeclarations(plan);
  }
  return failures;
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isOwner(owner) {
  if (!isRecord(owner)) return false;
  if (owner.kind === "platform") return Object.keys(owner).length === 1;
  return owner.kind === "module"
    && typeof owner.slug === "string"
    && owner.slug !== ""
    && owner.slug === owner.slug.trim()
    && Object.keys(owner).length === 2;
}

function isSafety(safety, name) {
  return safety === null || (
    isRecord(safety)
    && safety.migration === name
    && ["AdditiveOnly", "ExpandContract", "MaintenanceOnly", "Irreversible"].includes(safety.safety_class)
    && ["PreActivation", "PostActivation", "MaintenanceWindow"].includes(safety.phase_constraint)
    && typeof safety.requires_exclusive_lock === "boolean"
    && typeof safety.allows_concurrent_writes === "boolean"
    && typeof safety.backfill_required === "boolean"
    && Object.keys(safety).length === 6
  );
}

function validateDeclarations({ migrations, entries }) {
  if (migrations.length === 0) return ["migration declaration inventory must not be empty"];
  if (!Array.isArray(entries) || entries.length !== migrations.length) {
    return ["entries must contain one declaration for every migration"];
  }

  const failures = [];
  const preceding = new Set();
  for (const [index, name] of migrations.entries()) {
    const entry = entries[index];
    const label = `entries[${index}] (${JSON.stringify(name)})`;
    if (!isRecord(entry) || entry.migration !== name) {
      failures.push(`${label} must match the ordered migration name`);
    } else {
      if (Object.keys(entry).length !== 1 + declarationFields.length) {
        failures.push(`${label} must contain exactly the schema version 1 declaration fields`);
      }
      if (name !== name.trim()) failures.push(`${label} has an invalid migration name`);
      if (!isOwner(entry.owner)) failures.push(`${label} has an invalid owner`);
      if (!Array.isArray(entry.after) || new Set(entry.after).size !== entry.after.length
        || !entry.after.every((dependency) => preceding.has(dependency))) {
        failures.push(`${label} must declare unique dependencies that precede it`);
      }
      if (!isSafety(entry.safety, name)) failures.push(`${label} has invalid safety metadata`);
      if (entry.use_transaction !== null && typeof entry.use_transaction !== "boolean") {
        failures.push(`${label} has an invalid transaction declaration`);
      }
    }
    preceding.add(name);
  }
  return failures;
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

function compareProtectedPrefix(base, head) {
  const failures = comparePlans(base.migrations, head.migrations);
  if (failures.length > 0) return failures;

  for (const [index, before] of base.entries.entries()) {
    const after = head.entries[index];
    // The executor checks that all declared dependencies have run. The order
    // of edges within a declaration is immaterial to execution.
    const changed = declarationFields.filter((field) => field === "after"
      ? !isDeepStrictEqual(new Set(before.after), new Set(after.after))
      : !isDeepStrictEqual(before[field], after[field]));
    if (changed.length > 0) {
      failures.push(`protected migration ${JSON.stringify(before.migration)} changed declarations (${changed.join(", ")})`);
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
  const safety = {
    migration: "b",
    safety_class: "ExpandContract",
    phase_constraint: "PreActivation",
    requires_exclusive_lock: false,
    allows_concurrent_writes: true,
    backfill_required: true,
  };
  const base = {
    schema_version: 1,
    migrations: ["a", "b", "c"],
    entries: [
      { migration: "a", owner: { kind: "platform" }, after: [], safety: null, use_transaction: null },
      { migration: "b", owner: { kind: "module", slug: "blog" }, after: ["a"], safety, use_transaction: true },
      { migration: "c", owner: { kind: "module", slug: "forum" }, after: ["a", "b"], safety: null, use_transaction: false },
    ],
  };
  const namesOnly = { schema_version: 1, migrations: base.migrations };
  assert.deepEqual(validatePlan(namesOnly, false), []);
  assert.deepEqual(validatePlan(namesOnly, true), ["entries must contain one declaration for every migration"]);
  assert.deepEqual(validatePlan({ schema_version: 1, migrations: [], entries: [] }, true), [
    "migration declaration inventory must not be empty",
  ]);
  assert.deepEqual(validatePlan(base, true), []);

  const candidate = structuredClone(base);
  candidate.entries[2].after.reverse();
  candidate.migrations.push("d");
  candidate.entries.push({ ...candidate.entries[2], migration: "d", after: ["c"] });
  assert.deepEqual(validatePlan(candidate, true), []);
  assert.deepEqual(compareProtectedPrefix(base, candidate), []);
  assert.deepEqual(compareProtectedPrefix(candidate, base), ["head migration plan has 3 entries but base has 4"]);

  const rewrites = [
    ["owner", { kind: "module", slug: "forum" }],
    ["after", []],
    ["safety", { ...safety, phase_constraint: "PostActivation" }],
    ["safety", null],
    ["use_transaction", null],
    ["use_transaction", false],
  ];
  for (const [field, value] of rewrites) {
    const changed = structuredClone(candidate);
    changed.entries[1][field] = value;
    assert.deepEqual(validatePlan(changed, true), []);
    assert.deepEqual(compareProtectedPrefix(base, changed), [`protected migration "b" changed declarations (${field})`]);
  }

  const malformed = [
    ["migration", "c", "must match the ordered migration name"],
    ["owner", { kind: "module" }, "has an invalid owner"],
    ["after", ["a", "a"], "must declare unique dependencies that precede it"],
    ["after", ["c"], "must declare unique dependencies that precede it"],
    ["after", ["unknown"], "must declare unique dependencies that precede it"],
    ["safety", { ...safety, phase_constraint: "Unknown" }, "has invalid safety metadata"],
    ["use_transaction", "true", "has an invalid transaction declaration"],
  ];
  for (const [field, value, message] of malformed) {
    const changed = structuredClone(base);
    changed.entries[1][field] = value;
    assert.deepEqual(validatePlan(changed, true), [`entries[1] ("b") ${message}`]);
  }
  console.log("✔ migration plan compatibility self-test passed");
}

function main() {
  const args = process.argv.slice(2);
  const options = parseArguments(args);
  if (options.selfTest) {
    if (args.length !== 1) throw new Error("--self-test must be used on its own");
    runSelfTest();
    return;
  }
  if (!options.head || Boolean(options.base) === Boolean(options["protected-prefix"])) {
    throw new Error(
      "usage: verify-migration-plan-compatibility.mjs (--base FILE | --protected-prefix FILE) --head FILE",
    );
  }

  const protectedPrefix = options["protected-prefix"];
  const requireDeclarations = Boolean(protectedPrefix);
  const base = readPlan(protectedPrefix ?? options.base, requireDeclarations);
  const head = readPlan(options.head, requireDeclarations);
  const failures = requireDeclarations
    ? compareProtectedPrefix(base, head)
    : comparePlans(base.migrations, head.migrations);
  if (failures.length > 0) {
    console.error("Migration plan compatibility verification failed:");
    failures.forEach((failure) => console.error(`✗ ${failure}`));
    process.exit(Math.min(failures.length, 255));
  }

  const baseCount = base.migrations.length;
  const appended = head.migrations.length - baseCount;
  if (requireDeclarations) {
    console.log(`✔ protected migration declarations match (${baseCount} protected entries, ${appended} appended)`);
  } else {
    console.log(`✔ migration history is append-only (${baseCount} base entries, ${appended} appended)`);
  }
}

try {
  main();
} catch (error) {
  console.error(`Migration plan compatibility verification failed: ${error.message}`);
  process.exit(1);
}
