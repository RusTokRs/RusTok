#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const failures = [];

function readRegular(relativePath) {
  const file = path.join(repoRoot, relativePath);
  const stats = fs.lstatSync(file, { throwIfNoEntry: false });
  if (!stats) {
    failures.push(`${relativePath}: required file is missing`);
    return null;
  }
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${relativePath}: must be a regular non-symlink file`);
    return null;
  }
  return fs.readFileSync(file, "utf8");
}

const requestPath = "docs/ci/ruleset-activation-request.md";
const request = readRegular(requestPath);
if (request !== null) {
  for (const marker of [
    "repository-ruleset-admin-payload.json",
    "POST /repos/RusTokRs/RusTok/rulesets",
    "Migration harness approval",
    "Repository ruleset contract",
    "head SHA",
    "migration-infra-approved",
    "live `Repository Ruleset Contract` audit",
    "GitHub Actions integration `15368`",
    "strict freshness",
    "branch-creation enforcement",
    "Direct pushes to `main`, force pushes and branch deletion are rejected",
    "No permanent bypass actor is configured.",
    "positive and negative test pull requests",
  ]) {
    if (!request.includes(marker)) failures.push(`${requestPath}: missing marker ${marker}`);
  }
}

const statePath = "docs/ci/ruleset-activation-state.json";
const stateSource = readRegular(statePath);
if (stateSource !== null) {
  try {
    const state = JSON.parse(stateSource);
    const expected = {
      schema_version: 1,
      owner: "loid345",
      state: "pending_administrative_cutover",
      blocked_by: "current_direct_to_main_implementation_series",
      issue: 1837,
      contract: "docs/ci/repository-ruleset-contract.json",
      payload: "docs/ci/repository-ruleset-admin-payload.json",
      rollout: "docs/ci/main-protection-rollout.md",
      request: "docs/ci/ruleset-activation-request.md",
    };
    for (const [key, value] of Object.entries(expected)) {
      if (state[key] !== value) {
        failures.push(`${statePath}: ${key} must be ${JSON.stringify(value)}`);
      }
    }
    if (Object.keys(state).length !== Object.keys(expected).length) {
      failures.push(`${statePath}: unexpected or missing state fields`);
    }
  } catch (error) {
    failures.push(`${statePath}: invalid JSON: ${error.message}`);
  }
}

if (failures.length > 0) {
  console.error("Ruleset activation request verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ ruleset owner action, issue 1837, two required head-SHA checks, pending cutover state and acceptance criteria are bound",
);
