#!/usr/bin/env node
// Fast no-compile guardrails for rustok-iggy-connector lifecycle hardening.
// Checks that connector ack metadata has a canonical simulated + real SDK seam,
// that bundled mode manages a real native broker, and that subscriber ack paths
// validate token scope without transport policy.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];
const repoPath = (relativePath) => path.join(repoRoot, relativePath);
const readRepo = (relativePath) => readFileSync(repoPath(relativePath), "utf8");
const fail = (message) => failures.push(message);
const assertExists = (relativePath) => {
  if (!existsSync(repoPath(relativePath))) fail(`${relativePath}: expected file to exist`);
};
const assertContains = (text, pattern, description) => {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
};
const assertNotContains = (text, pattern, description) => {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
};

const libPath = "crates/rustok-iggy-connector/src/lib.rs";
const planPath = "crates/rustok-iggy-connector/docs/implementation-plan.md";
const docsPath = "crates/rustok-iggy-connector/docs/README.md";
const readmePath = "crates/rustok-iggy-connector/README.md";
const registryPath = "docs/modules/implementation-plans-registry.md";

for (const filePath of [libPath, planPath, docsPath, readmePath, registryPath]) assertExists(filePath);

const lib = readRepo(libPath);
const plan = readRepo(planPath);
const docs = readRepo(docsPath);
const readme = readRepo(readmePath);
const registry = readRepo(registryPath);

for (const marker of [
  "pub enum ConnectorAckToken",
  "Simulated {",
  "IggySdk {",
  "const IGGY_SDK_PREFIX",
  "pub fn iggy_sdk",
  "pub fn decode",
  "pub fn matches_scope",
  "ConnectorAckToken::simulated(mode, stream, topic, partition, offset).encode()",
  "ack token scope does not match external subscriber",
  "pub struct BundledConnector",
  "Bundled Iggy connector initialized",
  "async fn ensure_topology",
  "create_topic_if_not_exists",
  "fn connection_strings",
  "test_connector_ack_token_roundtrip_and_scope",
  "test_subscriber_ack_rejects_wrong_scope",
]) {
  assertContains(lib, marker, `${libPath}: missing connector ack guardrail marker ${marker}`);
}

assertNotContains(lib, /Some\("external:stream1:topic1:3:99"\)|assert_eq!\(token, "external:stream1:topic1:3:99"\)/, `${libPath}: simulated token tests must use canonical sim: prefix`);
assertNotContains(lib, /Some\("embedded:stream1:topic1:3:99"\)|assert_eq!\(token, "embedded:stream1:topic1:3:99"\)/, `${libPath}: embedded token tests must use canonical sim: prefix`);
assertNotContains(lib, "EmbeddedConnector", `${libPath}: legacy embedded connector must not remain`);
assertNotContains(lib, "LocalConnector", `${libPath}: legacy local connector name must not remain`);
assertNotContains(lib, "RemoteConnector", `${libPath}: legacy remote connector name must not remain`);

for (const text of [plan, docs, readme, registry]) {
  assertContains(text, "ConnectorAckToken", "docs/registry must mention ConnectorAckToken lifecycle seam");
  assertContains(text, "verify-iggy-connector-source.mjs", "docs/registry must mention no-compile verifier");
}

if (failures.length > 0) {
  console.error("iggy connector source verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("iggy connector source verification passed");
