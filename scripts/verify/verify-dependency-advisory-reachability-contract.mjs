#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--root") {
      const value = argv[index + 1];
      if (!value) throw new Error("--root requires a value");
      options.root = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  return options;
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const options = parseArguments(process.argv.slice(2));
const repoRoot = path.resolve(options.root || path.resolve(scriptDir, "../.."));
const workflowPath = ".github/workflows/dependency-advisory-reachability.yml";
const hardeningPath = ".github/workflows/hardening-gates.yml";
const verifierPath = "scripts/verify/verify-dependency-advisory-reachability-contract.mjs";
const failures = [];

function readRegularFile(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  const stats = fs.lstatSync(absolutePath, { throwIfNoEntry: false });
  if (!stats) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${relativePath}: must be a regular non-symlink file`);
    return "";
  }
  return fs.readFileSync(absolutePath, "utf8");
}

function requireMarkers(source, relativePath, markers) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${relativePath}: missing marker ${marker}`);
  }
}

function forbidMarkers(source, relativePath, markers) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relativePath}: forbidden marker ${marker}`);
  }
}

function requireOccurrenceCount(source, relativePath, marker, expected) {
  const actual = source.split(marker).length - 1;
  if (actual !== expected) {
    failures.push(`${relativePath}: expected ${expected} occurrences of ${marker}, found ${actual}`);
  }
}

const workflow = readRegularFile(workflowPath);
const hardening = readRegularFile(hardeningPath);
readRegularFile(verifierPath);

requireMarkers(workflow, workflowPath, [
  "name: Dependency Advisory Reachability",
  "  push:",
  "  pull_request:",
  "  workflow_dispatch:",
  "permissions:\n  contents: read",
  "cancel-in-progress: true",
  "runs-on: ubuntu-24.04",
  "timeout-minutes: 30",
  "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0",
  "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38",
  "node-version: 22",
  "rustup toolchain install 1.96.0 --profile minimal --no-self-update",
  "node scripts/verify/verify-dependency-feature-hygiene.mjs",
  "node scripts/verify/verify-advisory-exceptions.mjs",
  'grep -q \'^name = "rsa"$\' Cargo.lock',
  'grep -q \'^name = "atomic-polyfill"$\' Cargo.lock',
  "check_empty_tree()",
  "cargo tree --locked --workspace --all-features --target \"$target\" -i \"$package\"",
  "check_empty_tree rsa all",
  "thumbv6m-none-eabi",
  "riscv32i-unknown-none-elf",
  "riscv32imc-unknown-none-elf",
  "xtensa-esp32s2-none-elf",
  "cargo metadata --locked --all-features --format-version 1",
  `- ${verifierPath}`,
]);

requireOccurrenceCount(workflow, workflowPath, `- ${verifierPath}`, 2);

forbidMarkers(workflow, workflowPath, [
  "pull_request_target:",
  "contents: write",
  "actions: write",
  "checks: write",
  "issues: write",
  "pull-requests: write",
  "id-token: write",
  "persist-credentials: true",
  "cargo update",
  "cargo generate-lockfile",
  "cargo audit fix",
  "git push",
  "gh issue",
  "gh pr",
]);

requireMarkers(hardening, hardeningPath, [
  "Verify dependency advisory reachability structure",
  "node scripts/verify/verify-dependency-advisory-reachability-contract.mjs",
]);

forbidMarkers(hardening, hardeningPath, [
  "node scripts/verify/verify-dependency-advisory-reachability-contract.mjs --root /tmp",
]);

if (failures.length > 0) {
  console.error(`Dependency advisory reachability contract verification failed for ${repoRoot}:`);
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  `✔ dependency advisory reachability remains read-only, pinned, lock-aware and target-bounded in ${repoRoot}`,
);
