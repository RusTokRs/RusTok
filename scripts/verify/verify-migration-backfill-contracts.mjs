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
    if (["--base-plan", "--head-plan", "--contracts", "--output"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a file path`);
      options[argument.slice(2).replace(/-([a-z])/g, (_, character) => character.toUpperCase())] = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  return options;
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(path.resolve(file), "utf8"));
}

function validatePlan(plan, label) {
  const failures = [];
  if (plan.schema_version !== 1) failures.push(`${label}.schema_version must be 1`);
  if (!Array.isArray(plan.migrations)) {
    failures.push(`${label}.migrations must be an array`);
  } else {
    const names = new Set();
    for (const [index, name] of plan.migrations.entries()) {
      if (typeof name !== "string" || name.trim() === "") {
        failures.push(`${label}.migrations[${index}] must be a non-empty string`);
      } else if (names.has(name)) {
        failures.push(`${label}.migrations duplicates ${name}`);
      } else {
        names.add(name);
      }
    }
  }
  return failures;
}

function appendedMigrations(basePlan, headPlan) {
  const failures = [
    ...validatePlan(basePlan, "base plan"),
    ...validatePlan(headPlan, "head plan"),
  ];
  if (failures.length > 0) return { failures, appended: [] };

  if (headPlan.migrations.length < basePlan.migrations.length) {
    failures.push("head migration plan is shorter than base plan");
  }
  for (let index = 0; index < basePlan.migrations.length; index += 1) {
    if (basePlan.migrations[index] !== headPlan.migrations[index]) {
      failures.push(
        `migration ${index + 1} changed from ${JSON.stringify(basePlan.migrations[index])} to ${JSON.stringify(headPlan.migrations[index])}`,
      );
    }
  }

  return {
    failures,
    appended: failures.length
      ? []
      : headPlan.migrations.slice(basePlan.migrations.length),
  };
}

function validateContracts(register, headMigrations, appended) {
  const failures = [];
  const fixtures = [];
  if (register.schema_version !== 1) failures.push("contracts.schema_version must be 1");
  if (!Array.isArray(register.contracts)) {
    failures.push("contracts.contracts must be an array");
    return { failures, fixtures };
  }

  const ids = new Set();
  const migrations = new Map();
  const headNames = new Set(headMigrations);
  for (const [index, contract] of register.contracts.entries()) {
    const label = `contracts[${index}]`;
    for (const field of ["id", "migration", "mode", "owner", "reason"]) {
      if (typeof contract[field] !== "string" || contract[field].trim() === "") {
        failures.push(`${label}.${field} must be a non-empty string`);
      }
    }
    if (typeof contract.id === "string" && !/^[a-z0-9][a-z0-9._-]*$/.test(contract.id)) {
      failures.push(`${label}.id must use lowercase letters, digits, dot, underscore, or dash`);
    }
    if (ids.has(contract.id)) failures.push(`${label}.id duplicates ${contract.id}`);
    ids.add(contract.id);
    if (migrations.has(contract.migration)) {
      failures.push(`${label}.migration duplicates ${contract.migration}`);
    }
    migrations.set(contract.migration, contract);

    if (!headNames.has(contract.migration)) {
      failures.push(`${label}.migration is not present in the head migration plan`);
    }
    if (!['none', 'fixture'].includes(contract.mode)) {
      failures.push(`${label}.mode must be none or fixture`);
    }
    if (contract.mode === "fixture") {
      for (const field of ["setup_sql", "assertion_sql"]) {
        if (typeof contract[field] !== "string" || contract[field].trim() === "") {
          failures.push(`${label}.${field} must be non-empty for fixture mode`);
        }
      }
    } else if (contract.setup_sql !== undefined || contract.assertion_sql !== undefined) {
      failures.push(`${label}: none mode must not carry setup_sql or assertion_sql`);
    }
  }

  for (const migration of appended) {
    const contract = migrations.get(migration);
    if (!contract) {
      failures.push(`appended migration ${migration} has no backfill contract`);
      continue;
    }
    if (contract.mode === "fixture") {
      fixtures.push({
        id: contract.id,
        migration: contract.migration,
        setup_sql: contract.setup_sql,
        assertion_sql: contract.assertion_sql,
      });
    }
  }

  return { failures, fixtures };
}

function evaluate(basePlan, headPlan, register) {
  const planResult = appendedMigrations(basePlan, headPlan);
  if (planResult.failures.length > 0) {
    return { failures: planResult.failures, appended: [], fixtures: [] };
  }
  const contractResult = validateContracts(
    register,
    headPlan.migrations,
    planResult.appended,
  );
  return {
    failures: contractResult.failures,
    appended: planResult.appended,
    fixtures: contractResult.fixtures,
  };
}

function runSelfTest() {
  const base = { schema_version: 1, migrations: ["a"] };
  const head = { schema_version: 1, migrations: ["a", "b"] };
  const missing = evaluate(base, head, { schema_version: 1, contracts: [] });
  assert(missing.failures.includes("appended migration b has no backfill contract"));

  const none = evaluate(base, head, {
    schema_version: 1,
    contracts: [
      { id: "b-none", migration: "b", mode: "none", owner: "team", reason: "DDL only" },
    ],
  });
  assert.deepEqual(none.failures, []);
  assert.deepEqual(none.fixtures, []);

  const fixture = evaluate(base, head, {
    schema_version: 1,
    contracts: [
      {
        id: "b-backfill",
        migration: "b",
        mode: "fixture",
        owner: "team",
        reason: "Existing rows require conversion",
        setup_sql: "INSERT INTO legacy VALUES (1)",
        assertion_sql: "SELECT true AS passed",
      },
    ],
  });
  assert.deepEqual(fixture.failures, []);
  assert.equal(fixture.fixtures.length, 1);
  assert.equal(fixture.fixtures[0].migration, "b");

  const stale = evaluate(base, head, {
    schema_version: 1,
    contracts: [
      { id: "stale", migration: "missing", mode: "none", owner: "team", reason: "stale" },
      { id: "b-none", migration: "b", mode: "none", owner: "team", reason: "DDL only" },
    ],
  });
  assert(stale.failures.some((failure) => failure.includes("not present in the head migration plan")));

  console.log("✔ migration backfill contract self-test passed");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  if (!options.basePlan || !options.headPlan || !options.contracts || !options.output) {
    throw new Error(
      "usage: verify-migration-backfill-contracts.mjs --base-plan FILE --head-plan FILE --contracts FILE --output FILE",
    );
  }

  const result = evaluate(
    readJson(options.basePlan),
    readJson(options.headPlan),
    readJson(options.contracts),
  );
  if (result.failures.length > 0) {
    console.error("Migration backfill contract verification failed:");
    result.failures.forEach((failure) => console.error(`✗ ${failure}`));
    process.exit(Math.min(result.failures.length, 255));
  }

  const output = path.resolve(options.output);
  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.writeFileSync(
    output,
    `${JSON.stringify({ schema_version: 1, fixtures: result.fixtures }, null, 2)}\n`,
  );
  console.log(
    `✔ ${result.appended.length} appended migration(s) declared; ${result.fixtures.length} backfill fixture(s) selected`,
  );
}

try {
  main();
} catch (error) {
  console.error(`Migration backfill contract verification failed: ${error.message}`);
  process.exit(1);
}
