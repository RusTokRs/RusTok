#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function walk(relativePath) {
  const absolute = repoPath(relativePath);
  return readdirSync(absolute, { withFileTypes: true }).flatMap((entry) => {
    const child = path.posix.join(relativePath, entry.name);
    return entry.isDirectory() ? walk(child) : [child];
  });
}

function assert(condition, message) {
  if (!condition) failures.push(message);
}

const catalogPath = "crates/rustok-ai/contracts/rig-0.39-provider-catalog.json";
const cassettePath = "crates/rustok-ai/contracts/rig-0.39-stream-cassettes.json";
assert(existsSync(repoPath(catalogPath)), `missing ${catalogPath}`);
assert(existsSync(repoPath(cassettePath)), `missing ${cassettePath}`);

if (failures.length === 0) {
  const catalog = JSON.parse(read(catalogPath));
  const cassettes = JSON.parse(read(cassettePath));
  assert(catalog.rig_version === "0.39.0", "provider catalog must stay pinned to Rig 0.39.0");
  assert(cassettes.rig_version === catalog.rig_version, "stream cassettes must match provider catalog Rig version");
  const families = cassettes.cassettes.map((cassette) => cassette.family).sort();
  assert(
    JSON.stringify(families) === JSON.stringify(["anthropic", "cloud_auth", "deployment_local", "gemini", "openai_compatible"]),
    "stream cassette family coverage drift"
  );
}

const catalogSource = read("crates/rustok-ai/src/engine/catalog.rs");
const inferenceSource = read("crates/rustok-ai/src/engine/inference.rs");
assert(catalogSource.includes("enum ProviderIntegration"), "typed ProviderIntegration dispatch is required");
assert(catalogSource.includes("catalog_matches_the_rig_0_39_registry_snapshot"), "Rig snapshot test is required");
assert(inferenceSource.includes("match integration"), "inference factory must dispatch by ProviderIntegration");

const forbidden = ["ModelProvider", "OpenAiCompatibleProvider", "AnthropicProvider", "GeminiProvider", "AiRuntime"];
for (const relativePath of walk("crates/rustok-ai/src").filter((file) => file.endsWith(".rs"))) {
  const source = read(relativePath);
  for (const symbol of forbidden) {
    assert(!new RegExp(`\\b${symbol}\\b`).test(source), `${relativePath} retains forbidden legacy symbol ${symbol}`);
  }
  if (!relativePath.includes("/migrations/")) {
    assert(!source.includes("api_key_secret"), `${relativePath} retains plaintext secret storage marker`);
  }
}

if (failures.length > 0) {
  console.error("AI Rig cutover verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("AI Rig-only cutover verification passed");
