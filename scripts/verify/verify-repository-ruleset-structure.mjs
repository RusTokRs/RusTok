#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const file = path.join(repoRoot, relativePath);
  const stats = fs.lstatSync(file, { throwIfNoEntry: false });
  if (!stats) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
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
}

function forbidMarkers(relativePath, markers) {
  const source = read(relativePath);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relativePath}: forbidden marker ${marker}`);
  }
}

requireMarkers("docs/ci/repository-ruleset-contract.json", [
  '"schema_version": 1',
  '"repository": "RusTokRs/RusTok"',
  '"repository_id": 1144063896',
  '"branch": "main"',
  '"do_not_enforce_on_create": false',
  '"context": "Migration harness approval"',
  '"integration_slug": "github-actions"',
  '"integration_id": 15368',
  '"strict": true',
]);

requireMarkers("docs/ci/repository-ruleset-contract.md", [
  "Migration harness approval",
  "integration_id: 15368",
  "PR head SHA",
  "Require status checks to pass before merging",
  "Require branches to be up to date before merging",
  "Avoid permanent bypass actors",
  "Require workflows to pass before merging",
]);

const verifier = "scripts/verify/verify-repository-ruleset-contract.mjs";
requireMarkers(verifier, [
  'const API_VERSION = "2026-03-10"',
  "fs.lstatSync(file, { throwIfNoEntry: false })",
  "must be a regular non-symlink file",
  "required status check context duplicates",
  "integration_slug must be github-actions",
  "branch ${contract.branch} has no active required_status_checks rule",
  "must originate from integration",
  "must use strict branch freshness",
  "must enforce checks on branch creation",
  "appears ${matches.length} times",
  "/rules/branches/",
  '"X-GitHub-Api-Version": API_VERSION',
  'redirect: "error"',
  "process.env.RULESET_AUDIT_TOKEN || process.env.GITHUB_TOKEN || process.env.GH_TOKEN",
  "function runSelfTest",
]);
forbidMarkers(verifier, ["continue-on-error", "|| true", "redirect: \"follow\""]);

requireMarkers("scripts/verify/verify-repository-ruleset-self-test.mjs", [
  "verify-repository-ruleset-contract.mjs",
  '"--self-test"',
  "verify-repository-ruleset-structure.mjs",
]);

const approvalWorkflow = ".github/workflows/migration-infrastructure-approval.yml";
requireMarkers(approvalWorkflow, [
  "pull_request_target:",
  "permissions:\n  contents: read\n  checks: write",
  "Migration harness approval evaluator",
  "Checkout base policy source",
  "Checkout head policy as untrusted data",
  "allow-unsafe-pr-checkout: true",
  "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0",
  "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38",
  "Require approval for migration harness changes",
  "id: approval",
  "set +e",
  "exit_code=$?",
  'echo "exit_code=$exit_code" >> "$GITHUB_OUTPUT"',
  "Publish migration approval check on PR head",
  "HEAD_SHA: ${{ github.event.pull_request.head.sha }}",
  "EXIT_CODE: ${{ steps.approval.outputs.exit_code }}",
  '"/repos/$GITHUB_REPOSITORY/check-runs"',
  '-f name="Migration harness approval"',
  '-f head_sha="$HEAD_SHA"',
  '-f status="completed"',
  '-f conclusion="$conclusion"',
  "Enforce migration approval decision",
  "if: steps.approval.outputs.exit_code != '0'",
]);
forbidMarkers(approvalWorkflow, [
  "contents: write",
  "pull-requests: write",
  "statuses: write",
  "secrets:",
  'node "$GITHUB_WORKSPACE/head/',
  'bash "$GITHUB_WORKSPACE/head/',
  "uses: ./head",
  "continue-on-error: true",
  "|| true",
]);

const workflow = ".github/workflows/repository-ruleset-audit.yml";
requireMarkers(workflow, [
  "name: Repository Ruleset Contract",
  "pull_request_target:",
  "branches:\n      - main",
  "push:",
  "schedule:",
  'cron: "17 3 * * *"',
  "workflow_dispatch:",
  "permissions:\n  contents: read",
  "Repository ruleset contract",
  "Checkout base-owned ruleset policy",
  "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0",
  "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38",
  "persist-credentials: false",
  "Verify repository ruleset fixtures",
  "Audit active main branch rules",
  "base/scripts/verify/verify-repository-ruleset-contract.mjs",
  "base/docs/ci/repository-ruleset-contract.json",
  "GITHUB_TOKEN: ${{ github.token }}",
  '--repository "$GITHUB_REPOSITORY"',
  "--branch main",
]);
forbidMarkers(workflow, [
  "contents: write",
  "checks: write",
  "pull-requests: write",
  "secrets:",
  "Checkout head",
  'node "$GITHUB_WORKSPACE/head/',
  'bash "$GITHUB_WORKSPACE/head/',
  "uses: ./head",
  "continue-on-error: true",
  "|| true",
]);

requireMarkers("scripts/verify/verify-migration-infrastructure-approval.mjs", [
  ".github/workflows/repository-ruleset-audit.yml",
  ".github/workflows/hardening-gates.yml",
  "docs/ci/repository-ruleset-contract.json",
  "docs/ci/repository-ruleset-contract.md",
  "scripts/verify/verify-repository-ruleset-contract.mjs",
  "scripts/verify/verify-repository-ruleset-self-test.mjs",
  "scripts/verify/verify-repository-ruleset-structure.mjs",
  "scripts/verify/verify-all.sh",
]);

requireMarkers(".github/workflows/hardening-gates.yml", [
  "Verify repository ruleset contract fixtures",
  "verify-repository-ruleset-self-test.mjs",
]);
requireMarkers("scripts/verify/verify-all.sh", [
  "repository-ruleset-self-test  Verify required migration approval ruleset fixtures",
  "verify-repository-ruleset-self-test.mjs:Repository Ruleset Contract Fixtures",
]);

if (failures.length > 0) {
  console.error("Repository ruleset structure verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ strict GitHub-Actions-bound PR-head migration approval, base-owned live audit, deterministic fixtures and protected verification wiring are structurally bound",
);
