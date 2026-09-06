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
};
const crateName = moduleToCrate[moduleArg];

function fail(message) {
  console.error("[verify-page-builder-error-catalog-binding] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

if (!crateName) {
  fail(
    `unsupported module '${moduleArg}'. supported: ${Object.keys(moduleToCrate).join(", ")}`,
  );
}

function readFile(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolutePath)) {
    fail(`missing file: ${relativePath}`);
  }
  return fs.readFileSync(absolutePath, "utf8");
}

function extractBlock(content, header) {
  const start = content.indexOf(`[${header}]`);
  if (start === -1) {
    fail(`missing TOML block [${header}]`);
  }
  const rest = content.slice(start + header.length + 2);
  const nextHeader = rest.search(/^\[/m);
  return nextHeader === -1 ? rest : rest.slice(0, nextHeader);
}

function extractStringMap(content, header) {
  const block = extractBlock(content, header);
  const entries = new Map();
  for (const line of block.split(/\r?\n/)) {
    const match = line.match(/^\s*([A-Za-z0-9_-]+)\s*=\s*"([^"]+)"\s*$/);
    if (match) {
      entries.set(match[1], match[2]);
    }
  }
  return entries;
}

function assertMapContains(map, expected, label) {
  for (const [key, value] of Object.entries(expected)) {
    if (map.get(key) !== value) {
      fail(`${label}.${key} expected '${value}', got '${map.get(key) ?? "<missing>"}'`);
    }
  }
}

const providerManifest = readFile("crates/rustok-page-builder/rustok-module.toml");
const consumerManifest = readFile(`crates/${crateName}/rustok-module.toml`);
const consumerErrorRs = readFile(`crates/${crateName}/src/error.rs`);
const registry = JSON.parse(
  readFile("crates/rustok-page-builder/contracts/page-builder-fba-registry.json"),
);

const expectedCatalog = {
  validation: "validation",
  sanitize: "sanitize",
  runtime: "runtime",
  feature_disabled: "feature-disabled",
};
const expectedCodes = {
  feature_disabled: "FEATURE_DISABLED",
};
const expectedDegradedModeErrors = {
  builder_disabled: "FEATURE_DISABLED",
  preview_disabled: "FEATURE_DISABLED",
  publish_disabled: "FEATURE_DISABLED",
};

assertMapContains(
  extractStringMap(providerManifest, "fba.provider.error_catalog"),
  expectedCatalog,
  "provider error_catalog",
);
assertMapContains(
  extractStringMap(providerManifest, "fba.provider.error_codes"),
  expectedCodes,
  "provider error_codes",
);
assertMapContains(
  extractStringMap(consumerManifest, "fba.builder_consumer.error_catalog"),
  expectedCatalog,
  "consumer error_catalog",
);
assertMapContains(
  extractStringMap(consumerManifest, "fba.builder_consumer.error_codes"),
  expectedCodes,
  "consumer error_codes",
);
assertMapContains(
  extractStringMap(consumerManifest, "fba.builder_consumer.degraded_mode_errors"),
  expectedDegradedModeErrors,
  "consumer degraded_mode_errors",
);

for (const token of [
  "BUILDER_RUNTIME_ERROR_CATALOG",
  "BUILDER_FEATURE_DISABLED_ERROR_CODE",
  "feature-disabled",
]) {
  if (!consumerErrorRs.includes(token)) {
    fail(`consumer runtime error catalog source is missing '${token}'`);
  }
}

assertMapContains(
  new Map(Object.entries(registry.provider.error_catalog ?? {})),
  expectedCatalog,
  "registry provider error_catalog",
);
assertMapContains(
  new Map(Object.entries(registry.provider.error_codes ?? {})),
  expectedCodes,
  "registry provider error_codes",
);
const registryConsumer = (registry.consumers ?? []).find(
  (consumer) => consumer.module_slug === moduleArg,
);
if (!registryConsumer) {
  fail(`registry missing consumer '${moduleArg}'`);
}
assertMapContains(
  new Map(Object.entries(registryConsumer.error_catalog ?? {})),
  expectedCatalog,
  "registry consumer error_catalog",
);
assertMapContains(
  new Map(Object.entries(registryConsumer.error_codes ?? {})),
  expectedCodes,
  "registry consumer error_codes",
);
assertMapContains(
  new Map(Object.entries(registryConsumer.degraded_mode_errors ?? {})),
  expectedDegradedModeErrors,
  "registry consumer degraded_mode_errors",
);

console.log("[verify-page-builder-error-catalog-binding] PASS");
console.log(`module=${moduleArg}; crate=${crateName}`);
