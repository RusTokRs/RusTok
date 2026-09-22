#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const relativePath = "docs/ci/main-protection-rollout.md";
const file = path.join(repoRoot, relativePath);
const stats = fs.lstatSync(file, { throwIfNoEntry: false });
const failures = [];

if (!stats) {
  failures.push(`${relativePath}: required file is missing`);
} else if (!stats.isFile() || stats.isSymbolicLink()) {
  failures.push(`${relativePath}: must be a regular non-symlink file`);
} else {
  const source = fs.readFileSync(file, "utf8");
  const required = [
    "Finish the currently authorized direct-to-`main` implementation series.",
    "Require a pull request before merging.",
    "Require status checks to pass before merging.",
    "Require branches to be up to date before merging.",
    "Migration harness approval",
    "Repository ruleset contract",
    "integration_id: 15368",
    "do_not_enforce_on_create",
    "Block force pushes and branch deletion.",
    "Require conversation resolution before merging.",
    "Do not configure permanent bypass actors.",
    "successful head-SHA `Migration harness approval` and `Repository ruleset contract` Check Runs",
    "changes a protected migration file without the approval label",
    "Apply `migration-infra-approved`",
    "repository-ruleset-admin-payload.json",
    "Rerun `Repository Ruleset Contract` manually",
    "both required checks are attached to the latest PR head SHA",
    "Make pull requests the only normal delivery path",
    "Use a time-bounded organization or repository owner bypass.",
    "Remove the temporary bypass immediately.",
  ];
  for (const marker of required) {
    if (!source.includes(marker)) failures.push(`${relativePath}: missing marker ${marker}`);
  }
}

if (failures.length > 0) {
  console.error("Main protection rollout verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ PR-only main protection rollout, two required head-SHA checks, negative approval test and time-bounded recovery are documented",
);
