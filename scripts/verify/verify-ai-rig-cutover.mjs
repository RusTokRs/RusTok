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

const catalogPath = "crates/modules/rustok-ai/contracts/provider-catalog.json";
const cassettePath = "crates/modules/rustok-ai/contracts/stream-cassettes.json";
assert(existsSync(repoPath(catalogPath)), `missing ${catalogPath}`);
assert(existsSync(repoPath(cassettePath)), `missing ${cassettePath}`);

if (failures.length === 0) {
  const catalog = JSON.parse(read(catalogPath));
  const cassettes = JSON.parse(read(cassettePath));
  const manifest = read("Cargo.toml");
  const rigPin = manifest.match(/^rig\s*=\s*\{[^\n]*version\s*=\s*"=([^"]+)"/m)?.[1];
  assert(Boolean(rigPin), "workspace Rig must have an exact version pin");
  assert(catalog.rig_version === rigPin, "provider catalog must match the workspace Rig pin");
  for (const companion of ["rig-bedrock", "rig-fastembed", "rig-gemini-grpc", "rig-vertexai"]) {
    const pin = manifest.match(new RegExp(`^${companion}\\s*=\\s*"=([^"]+)"`, "m"))?.[1];
    assert(pin === rigPin, `${companion} must match the workspace Rig pin`);
  }
  assert(cassettes.rig_version === catalog.rig_version, "stream cassettes must match provider catalog Rig version");
  const families = cassettes.cassettes.map((cassette) => cassette.family).sort();
  assert(
    JSON.stringify(families) === JSON.stringify(["anthropic", "cloud_auth", "deployment_local", "gemini", "openai_compatible"]),
    "stream cassette family coverage drift"
  );
}

const catalogSource = read("crates/modules/rustok-ai/src/engine/catalog.rs");
const inferenceSource = read("crates/modules/rustok-ai/src/engine/inference.rs");
assert(catalogSource.includes("enum ProviderIntegration"), "typed ProviderIntegration dispatch is required");
assert(inferenceSource.includes("match integration"), "inference factory must dispatch by ProviderIntegration");

const forbidden = ["ModelProvider", "OpenAiCompatibleProvider", "AnthropicProvider", "GeminiProvider", "AiRuntime"];
for (const relativePath of walk("crates/modules/rustok-ai/src").filter((file) => file.endsWith(".rs"))) {
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
