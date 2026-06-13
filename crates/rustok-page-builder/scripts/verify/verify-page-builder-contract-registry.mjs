#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

const registryPath = path.join(
  repoRoot,
  "crates",
  "rustok-page-builder",
  "contracts",
  "page-builder-fba-registry.json",
);
const providerManifestPath = path.join(
  repoRoot,
  "crates",
  "rustok-page-builder",
  "rustok-module.toml",
);

const moduleArg = process.argv[2] ?? "pages";

function fail(messages) {
  console.error("[verify-page-builder-contract-registry] FAIL");
  for (const message of Array.isArray(messages) ? messages : [messages]) {
    console.error(`- ${message}`);
  }
  process.exit(1);
}

function readFile(filePath) {
  if (!fs.existsSync(filePath)) {
    fail(`missing file: ${path.relative(repoRoot, filePath)}`);
  }
  return fs.readFileSync(filePath, "utf8");
}

function readJson(filePath) {
  try {
    return JSON.parse(readFile(filePath));
  } catch (error) {
    fail(`invalid JSON in ${path.relative(repoRoot, filePath)}: ${error.message}`);
  }
}

function extractString(content, key, label) {
  const match = content.match(new RegExp(`^\\s*${key}\\s*=\\s*"([^"]+)"\\s*$`, "m"));
  if (!match?.[1]) {
    fail(`cannot extract ${label} (${key})`);
  }
  return match[1];
}

function extractArray(content, key, label) {
  const match = content.match(new RegExp(`^\\s*${key}\\s*=\\s*\\[([^\\]]*)\\]`, "m"));
  if (!match?.[1]) {
    fail(`cannot extract ${label} (${key})`);
  }
  return [...match[1].matchAll(/"([^"]+)"/g)].map((item) => item[1]);
}

function assertSame(errors, label, expected, actual) {
  if (expected !== actual) {
    errors.push(`${label} mismatch: registry=${expected}, manifest=${actual}`);
  }
}

function assertArraySame(errors, label, expected, actual) {
  const expectedKey = [...expected].sort().join(",");
  const actualKey = [...actual].sort().join(",");
  if (expectedKey !== actualKey) {
    errors.push(`${label} mismatch: registry=[${expectedKey}], manifest=[${actualKey}]`);
  }
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

const registry = readJson(registryPath);
const providerManifest = readFile(providerManifestPath);
const provider = registry.provider;
const errors = [];

if (registry.schema_version !== 1) {
  errors.push(`unsupported schema_version: ${registry.schema_version}`);
}

assertSame(
  errors,
  "provider.module_slug",
  provider.module_slug,
  extractString(providerManifest, "slug", "provider module slug"),
);
assertSame(
  errors,
  "provider.contract",
  provider.contract,
  extractString(providerManifest, "contract", "provider contract"),
);
assertSame(
  errors,
  "provider.builder_contract_version",
  provider.builder_contract_version,
  extractString(providerManifest, "builder_contract_version", "provider builder_contract_version"),
);
assertSame(
  errors,
  "provider.consumer_min_version",
  provider.consumer_min_version,
  extractString(providerManifest, "consumer_min_version", "provider consumer_min_version"),
);
assertArraySame(
  errors,
  "provider.capabilities",
  provider.capabilities,
  extractArray(providerManifest, "capabilities", "provider capabilities"),
);
assertSame(
  errors,
  "provider.health_profile",
  provider.health_profile,
  extractString(providerManifest, "health_profile", "provider health_profile"),
);
assertArraySame(
  errors,
  "provider.health_states",
  provider.health_states,
  extractArray(providerManifest, "health_states", "provider health_states"),
);
assertArraySame(
  errors,
  "provider.degradation_reasons",
  provider.degradation_reasons,
  extractArray(providerManifest, "degradation_reasons", "provider degradation_reasons"),
);

if (!["ready", "degraded", "unavailable"].includes(provider.health_profile)) {
  errors.push(`provider.health_profile has unsupported value: ${provider.health_profile}`);
}
for (const state of ["ready", "degraded", "unavailable"]) {
  if (!provider.health_states?.includes(state)) {
    errors.push(`provider.health_states missing '${state}'`);
  }
}
for (const key of [
  "preview_p95_ms",
  "publish_p95_ms",
  "sanitize_failure_rate_max",
  "runtime_error_rate_max",
]) {
  if (!(key in (provider.slo_thresholds ?? {}))) {
    errors.push(`provider.slo_thresholds missing '${key}'`);
  }
}
for (const [key, value] of Object.entries(provider.slo_thresholds ?? {})) {
  if (typeof value !== "number" || value < 0) {
    errors.push(`provider.slo_thresholds.${key} must be a non-negative number`);
  }
}

const consumers = moduleArg === "all"
  ? registry.consumers
  : registry.consumers.filter((consumer) => consumer.module_slug === moduleArg);

if (consumers.length === 0) {
  fail(`module '${moduleArg}' is not declared in ${path.relative(repoRoot, registryPath)}`);
}

for (const consumer of consumers) {
  const manifestPath = path.join(repoRoot, "crates", consumer.crate, "rustok-module.toml");
  const manifest = readFile(manifestPath);

  assertSame(
    errors,
    `${consumer.module_slug}.module_slug`,
    consumer.module_slug,
    extractString(manifest, "slug", `${consumer.module_slug} module slug`),
  );
  assertSame(
    errors,
    `${consumer.module_slug}.provider_module`,
    consumer.provider_module,
    extractString(manifest, "provider_module", `${consumer.module_slug} provider_module`),
  );
  assertSame(
    errors,
    `${consumer.module_slug}.contract`,
    consumer.contract,
    extractString(manifest, "contract", `${consumer.module_slug} contract`),
  );
  assertSame(
    errors,
    `${consumer.module_slug}.contract_version`,
    consumer.contract_version,
    extractString(manifest, "contract_version", `${consumer.module_slug} contract_version`),
  );
  assertSame(
    errors,
    `${consumer.module_slug}.builder_contract_version`,
    consumer.builder_contract_version,
    extractString(
      manifest,
      "builder_contract_version",
      `${consumer.module_slug} builder_contract_version`,
    ),
  );
  assertSame(
    errors,
    `${consumer.module_slug}.consumer_min_version`,
    consumer.consumer_min_version,
    extractString(manifest, "consumer_min_version", `${consumer.module_slug} consumer_min_version`),
  );
  assertArraySame(
    errors,
    `${consumer.module_slug}.capabilities`,
    consumer.capabilities,
    extractArray(manifest, "capabilities", `${consumer.module_slug} capabilities`),
  );

  const consumerVersion = parseVersion(
    consumer.builder_contract_version,
    `${consumer.module_slug}.builder_contract_version`,
  );
  const providerMinVersion = parseVersion(provider.consumer_min_version, "provider.consumer_min_version");
  if (compareVersions(consumerVersion, providerMinVersion) < 0) {
    errors.push(
      `${consumer.module_slug}.builder_contract_version=${consumer.builder_contract_version} is below provider.consumer_min_version=${provider.consumer_min_version}`,
    );
  }
}

if (errors.length > 0) {
  fail(errors);
}

console.log("[verify-page-builder-contract-registry] PASS");
console.log(
  `registry=${path.relative(repoRoot, registryPath)}; provider=${provider.builder_contract_version}; module=${moduleArg}`,
);
