#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const relativePath = "docs/ci/ruleset-activation-request.md";
const file = path.join(repoRoot, relativePath);
const stats = fs.lstatSync(file, { throwIfNoEntry: false });
const failures = [];

if (!stats) {
  failures.push(`${relativePath}: required file is missing`);
} else if (!stats.isFile() || stats.isSymbolicLink()) {
  failures.push(`${relativePath}: must be a regular non-symlink file`);
} else {
  const source = fs.readFileSync(file, "utf8");
  for (const marker of [
    "repository-ruleset-admin-payload.json",
    "POST /repos/RusTokRs/RusTok/rulesets",
    "Migration harness approval",
    "head SHA",
    "migration-infra-approved",
    "Repository Ruleset Contract",
    "Direct pushes to `main`, force pushes and branch deletion are rejected",
    "No permanent bypass actor is configured.",
    "positive and negative test pull requests",
  ]) {
    if (!source.includes(marker)) failures.push(`${relativePath}: missing marker ${marker}`);
  }
}

if (failures.length > 0) {
  console.error("Ruleset activation request verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ ruleset owner action and acceptance criteria are documented");
