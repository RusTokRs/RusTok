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

function requireFile(relativePath) {
  if (!fs.existsSync(path.join(repoRoot, relativePath))) {
    failures.push(`${relativePath}: required file is missing`);
    return false;
  }
  return true;
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

const smokeTest = "crates/rustok-migrations/tests/postgres_zero_migration_smoke.rs";
requireMarkers(smokeTest, [
  'env_binary_flag("RUSTOK_MIGRATION_SMOKE_REUSE_DB")',
  "if reuse_database {",
  "reused migration smoke database",
  "must already exist and be reachable",
  "drop_database_if_exists(&admin, &database_name).await?;",
  "create_database(&admin, &database_name).await?;",
  "apply_migrations_and_assert_schema(&target_url, incremental)",
  "apply_migrations_incrementally(&db)",
  "Migrator::get_pending_migrations(&db)",
  'assert_trigger_exists(&db, "trg_products_normalize_channel_visibility")',
  'parse_binary_flag("RUSTOK_MIGRATION_SMOKE_REUSE_DB", Some("true"))',
]);

const smokeScript = "scripts/verify/verify-migration-smoke.sh";
requireMarkers(smokeScript, [
  "RUSTOK_MIGRATION_SMOKE_REUSE_DB",
  'mode="reuse-upgrade-incremental"',
  "cargo test --locked -p rustok-migrations",
  "postgres_zero_migration_smoke_applies_from_empty_database",
]);
forbidMarkers(smokeScript, ["-p migration ", "|| true"]);

requireMarkers("scripts/verify/verify-migration-infrastructure-approval.mjs", [
  'const APPROVAL_LABEL = "migration-infra-approved"',
  "const PROTECTED_PATHS",
  ".github/workflows/migration-compatibility.yml",
  "postgres_zero_migration_smoke.rs",
  "verify-migration-compatibility-contract.mjs",
  "verify-migration-infra-self-test.mjs",
  "function changedProtectedPaths",
  "function approvalDecision",
  "function runSelfTest",
]);
requireMarkers("scripts/verify/verify-migration-infra-self-test.mjs", [
  "verify-migration-infrastructure-approval.mjs",
  '"--self-test"',
]);

const workflow = ".github/workflows/migration-compatibility.yml";
requireMarkers(workflow, [
  "name: Migration Compatibility",
  "pull_request_target:",
  "allow_infrastructure_changes:",
  "permissions:\n  contents: read",
  "persist-credentials: false",
  "Migration harness approval",
  "timeout-minutes: 5",
  "Verify base approval policy fixtures",
  "Require approval for migration harness changes",
  "migration-infra-approved",
  "base/scripts/verify/verify-migration-infrastructure-approval.mjs",
  "needs: infrastructure-approval",
  "image: postgres:16",
  "timeout-minutes: 35",
  "timeout-minutes: 45",
  "Fresh PostgreSQL (${{ matrix.name }})",
  "PostgreSQL N-1 to head upgrade",
  "name: apply-all",
  "name: incremental",
  "Checkout base migration source",
  "Checkout head migration source",
  "Apply base migrations and preserve database",
  "Upgrade preserved database with head migrations",
  'RUSTOK_MIGRATION_SMOKE_KEEP_DB: "1"',
  'RUSTOK_MIGRATION_SMOKE_REUSE_DB: "1"',
  'RUSTOK_MIGRATION_SMOKE_INCREMENTAL: "1"',
  'manifest-path "$GITHUB_WORKSPACE/base/Cargo.toml"',
  'manifest-path "$GITHUB_WORKSPACE/head/Cargo.toml"',
  "-p rustok-migrations",
  "--locked",
  "target/migration-base",
  "target/migration-head",
]);
forbidMarkers(workflow, [
  "\n  pull_request:\n",
  "continue-on-error: true",
  "|| true",
  'bash "$GITHUB_WORKSPACE/base/scripts/verify/verify-migration-smoke.sh"',
]);

if (failures.length > 0) {
  console.error("Migration compatibility contract verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ PostgreSQL fresh, incremental, and N-1 migration compatibility paths are structurally bound",
);
