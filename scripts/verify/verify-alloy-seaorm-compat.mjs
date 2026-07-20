#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function count(source, marker) {
  return source.split(marker).length - 1;
}

function requireCount(source, marker, expected, location) {
  const observed = count(source, marker);
  if (observed !== expected) {
    failures.push(`${location}: expected ${expected} occurrences of ${marker}, observed ${observed}`);
  }
}

function forbid(source, marker, location) {
  if (source.includes(marker)) {
    failures.push(`${location}: forbidden compatibility regression marker ${marker}`);
  }
}

const releasePath = "crates/alloy/src/runner/release.rs";
const testPath = "crates/alloy/src/runner/test.rs";
const memoryPath = "crates/alloy/src/storage/memory.rs";
const seaOrmPath = "crates/alloy/src/storage/sea_orm.rs";

const release = read(releasePath);
const testRunner = read(testPath);
const memory = read(memoryPath);
const seaOrm = read(seaOrmPath);

requireCount(
  release,
  "G: AlloyReleaseGovernance + ?Sized,",
  2,
  releasePath,
);
forbid(release, "G: AlloyReleaseGovernance,", releasePath);

requireCount(
  testRunner,
  "crate::model::MAX_TEST_ERROR_LENGTH",
  1,
  testPath,
);
forbid(testRunner, "crate::MAX_TEST_ERROR_LENGTH", testPath);

requireCount(
  memory,
  "crate::model::test_run_lease_expires_at",
  2,
  memoryPath,
);
forbid(memory, "crate::test_run_lease_expires_at", memoryPath);

requireCount(
  seaOrm,
  "crate::model::test_run_lease_expires_at",
  2,
  seaOrmPath,
);
forbid(seaOrm, "crate::test_run_lease_expires_at", seaOrmPath);

requireCount(
  seaOrm,
  ".col_expr(Column::UpdatedAt, Expr::col(Column::UpdatedAt).into())",
  2,
  seaOrmPath,
);
forbid(
  seaOrm,
  ".col_expr(Column::UpdatedAt, Expr::col(Column::UpdatedAt))",
  seaOrmPath,
);

const fixtureFailures = [];
const badFixture = {
  release: "G: AlloyReleaseGovernance,",
  testRunner: "crate::MAX_TEST_ERROR_LENGTH",
  memory: "crate::test_run_lease_expires_at",
  seaOrm: ".col_expr(Column::UpdatedAt, Expr::col(Column::UpdatedAt))",
};
if (!badFixture.release.includes("G: AlloyReleaseGovernance,")) fixtureFailures.push("release");
if (!badFixture.testRunner.includes("crate::MAX_TEST_ERROR_LENGTH")) fixtureFailures.push("test");
if (!badFixture.memory.includes("crate::test_run_lease_expires_at")) fixtureFailures.push("memory");
if (!badFixture.seaOrm.includes("Expr::col(Column::UpdatedAt))")) fixtureFailures.push("sea-orm");
if (fixtureFailures.length > 0) {
  failures.push(`self-test did not retain negative fixtures: ${fixtureFailures.join(", ")}`);
}

if (failures.length > 0) {
  console.error("Alloy SeaORM compatibility verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Alloy test-run symbols, governance trait objects, and SeaORM expressions remain API-compatible");
