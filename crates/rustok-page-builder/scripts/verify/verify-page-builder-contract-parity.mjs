#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

const moduleArg = process.argv[2] ?? "pages";
const moduleToCrate = {
  pages: "rustok-pages",
  forum: "rustok-forum",
};
const crateName = moduleToCrate[moduleArg];

function fail(messages) {
  console.error("[verify-page-builder-contract-parity] FAIL");
  for (const message of Array.isArray(messages) ? messages : [messages]) {
    console.error(`- ${message}`);
  }
  process.exit(1);
}

if (!crateName) {
  fail(`unsupported module '${moduleArg}'. supported: ${Object.keys(moduleToCrate).join(", ")}`);
}

const providerManifest = path.join(repoRoot, "crates", "rustok-page-builder", "rustok-module.toml");
const consumerManifest = path.join(repoRoot, "crates", crateName, "rustok-module.toml");

function readFile(filePath) {
  if (!fs.existsSync(filePath)) {
    fail(`missing file: ${path.relative(repoRoot, filePath)}`);
  }
  return fs.readFileSync(filePath, "utf8");
}

function extractVersion(content, pattern, label) {
  const match = content.match(pattern);
  if (!match?.[1]) {
    fail(`cannot extract ${label}`);
  }
  return match[1];
}

function parseVersion(version, label) {
  const parts = version.split(".");
  if (parts.length === 0) {
    fail(`invalid numeric version segment in ${label}: ${version}`);
  }

  return parts.map((part) => {
    if (!/^\d+$/.test(part)) {
      fail(`invalid numeric version segment in ${label}: ${version}`);
    }
    return Number.parseInt(part, 10);
  });
}

function compareVersions(left, right) {
  const width = Math.max(left.length, right.length);
  for (let index = 0; index < width; index += 1) {
    const l = left[index] ?? 0;
    const r = right[index] ?? 0;
    if (l !== r) return l > r ? 1 : -1;
  }
  return 0;
}

const provider = readFile(providerManifest);
const consumer = readFile(consumerManifest);

const providerVersion = extractVersion(
  provider,
  /^\s*builder_contract_version\s*=\s*"([^"]+)"\s*$/m,
  "provider builder_contract_version",
);
const providerConsumerMinVersion = extractVersion(
  provider,
  /^\s*consumer_min_version\s*=\s*"([^"]+)"\s*$/m,
  "provider consumer_min_version",
);
const consumerVersion = extractVersion(
  consumer,
  /^\s*builder_contract_version\s*=\s*"([^"]+)"\s*$/m,
  "consumer builder_contract_version",
);
const consumerMinVersion = extractVersion(
  consumer,
  /^\s*consumer_min_version\s*=\s*"([^"]+)"\s*$/m,
  "consumer consumer_min_version",
);
const consumerContractVersion = extractVersion(
  consumer,
  /^\s*contract_version\s*=\s*"([^"]+)"\s*$/m,
  "consumer contract_version",
);

const errors = [];

if (providerVersion !== consumerVersion) {
  errors.push(
    `builder_contract_version mismatch: provider=${providerVersion}, consumer=${consumerVersion}`,
  );
}

if (providerVersion !== consumerContractVersion) {
  errors.push(
    `consumer contract_version mismatch: provider=${providerVersion}, consumer_contract_version=${consumerContractVersion}`,
  );
}

if (providerConsumerMinVersion !== consumerMinVersion) {
  errors.push(
    `consumer_min_version mismatch: provider=${providerConsumerMinVersion}, consumer=${consumerMinVersion}`,
  );
}

if (
  compareVersions(
    parseVersion(consumerVersion, "consumer builder_contract_version"),
    parseVersion(providerConsumerMinVersion, "provider consumer_min_version"),
  ) < 0
) {
  errors.push(
    `consumer builder_contract_version ${consumerVersion} is below provider consumer_min_version ${providerConsumerMinVersion}`,
  );
}

if (errors.length > 0) {
  fail(errors);
}

console.log("[verify-page-builder-contract-parity] PASS");
console.log(
  `module=${moduleArg}; provider=${providerVersion}; consumer=${consumerVersion}; consumer_contract_version=${consumerContractVersion}; consumer_min_version=${consumerMinVersion}`,
);
