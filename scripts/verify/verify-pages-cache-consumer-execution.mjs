#!/usr/bin/env node

import { existsSync, lstatSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = "crates/rustok-pages/contracts/evidence/pages-cache-consumer-execution.json";
const workflowPath = ".github/workflows/pages-cache-consumer-execution-evidence.yml";
const recorderPath = "scripts/evidence/record-pages-cache-consumer-execution.mjs";

function fail(message) {
  throw new Error(`Pages cache-consumer execution verifier failed: ${message}`);
}

function file(relativePath) {
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) fail(`${relativePath} escapes repository root`);
  if (!existsSync(absolute)) fail(`${relativePath} is missing`);
  const metadata = lstatSync(absolute);
  if (metadata.isSymbolicLink() || !metadata.isFile()) fail(`${relativePath} must be a regular file`);
  return readFileSync(absolute, "utf8");
}

function json(relativePath) {
  try {
    const document = JSON.parse(file(relativePath));
    if (document === null || typeof document !== "object" || Array.isArray(document)) {
      fail(`${relativePath} must contain a JSON object`);
    }
    return document;
  } catch (error) {
    if (error instanceof SyntaxError) fail(`${relativePath} is invalid JSON: ${error.message}`);
    throw error;
  }
}

function pointerValue(document, pointer) {
  if (typeof pointer !== "string" || !pointer.startsWith("/")) fail("target JSON Pointer is invalid");
  let current = document;
  for (const rawToken of pointer.slice(1).split("/")) {
    const token = rawToken.replaceAll("~1", "/").replaceAll("~0", "~");
    if (current === null || typeof current !== "object" || !Object.hasOwn(current, token)) {
      fail(`target JSON Pointer does not resolve at ${rawToken}`);
    }
    current = current[token];
  }
  return current;
}

function requireIncludes(text, fragment, label) {
  if (!text.includes(fragment)) fail(`${label} is missing: ${fragment}`);
}

function main() {
  const contract = json(contractPath);
  if (contract.format !== "pages_cache_consumer_execution_source_v1") fail("contract format drifted");
  if (contract.status !== "source_ready_main_execution_pending") fail("contract status drifted");
  if (contract.target?.path !== "crates/rustok-page-builder/contracts/page-builder-fba-registry.json") {
    fail("target registry path drifted");
  }
  if (contract.target?.executed_evidence_json_pointer !== "/consumers/0/cache_consumer/executed_evidence") {
    fail("cache consumer target pointer drifted");
  }
  if (contract.target?.required_before_value !== "pending") fail("target pre-state must remain pending");

  const registry = json(contract.target.path);
  if (registry.status !== contract.target.required_registry_status) fail("FBA registry status drifted");
  if (pointerValue(registry, contract.target.executed_evidence_json_pointer) !== "pending") {
    fail("cache consumer executed_evidence is no longer pending");
  }

  const packets = Object.values(contract.source_packets ?? {});
  if (packets.length < 7) fail("source packet set is incomplete");
  for (const packet of packets) {
    if (typeof packet.path !== "string" || typeof packet.required_status !== "string") {
      fail("source packet shape is invalid");
    }
    const source = json(packet.path);
    if (source.status !== packet.required_status) fail(`source packet status drifted: ${packet.path}`);
  }

  const verifierCommands = contract.execution?.verifier_commands;
  const testCommands = contract.execution?.test_commands;
  const checkCommands = contract.execution?.check_commands;
  if (!Array.isArray(verifierCommands) || verifierCommands.length !== 10) fail("expected ten verifier commands");
  if (!Array.isArray(testCommands) || testCommands.length !== 9) fail("expected nine test commands");
  if (!Array.isArray(checkCommands) || checkCommands.length !== 3) fail("expected three cargo check commands");
  if (contract.execution?.postgres?.required !== true) fail("PostgreSQL execution must remain required");
  if (contract.output?.format !== "pages_cache_consumer_execution_v1") fail("output format drifted");
  if (contract.output?.success_status !== "cache_consumer_execution_passed_admission_pending") {
    fail("output success status drifted");
  }

  if (!Array.isArray(contract.required_source_files) || contract.required_source_files.length < 30) {
    fail("required source hash set is incomplete");
  }
  for (const required of contract.required_source_files) file(required);

  const workflow = file(workflowPath);
  requireIncludes(workflow, "name: Pages Cache Consumer Execution Evidence", "workflow identity");
  requireIncludes(workflow, "pull_request:", "PR validation trigger");
  requireIncludes(workflow, "push:", "main evidence trigger");
  requireIncludes(workflow, "- main", "main branch restriction");
  requireIncludes(workflow, "contents: read", "workflow permissions");
  requireIncludes(workflow, "image: postgres:16-alpine", "PostgreSQL service");
  requireIncludes(workflow, "RUSTOK_PAGES_TEST_DATABASE_URL", "PostgreSQL test environment");
  requireIncludes(workflow, "node scripts/evidence/record-pages-cache-consumer-execution.mjs", "main recorder step");
  requireIncludes(workflow, "actions/upload-artifact@v7", "receipt artifact upload");
  requireIncludes(workflow, "github.event_name == 'push' && github.ref == 'refs/heads/main'", "main receipt condition");
  for (const command of [...verifierCommands, ...testCommands, ...checkCommands]) {
    requireIncludes(workflow, command, `workflow command ${command}`);
  }

  const recorder = file(recorderPath);
  requireIncludes(recorder, "Pages Cache Consumer Execution Evidence", "recorder workflow identity");
  requireIncludes(recorder, 'eventName !== "push" || refName !== "main"', "recorder exact-main check");
  requireIncludes(recorder, "cache_consumer_execution_passed_admission_pending", "recorder success status");
  requireIncludes(recorder, "fba_registry_mutated: false", "recorder registry state");
  requireIncludes(recorder, "cross_process_exact_once_claimed: false", "recorder exact-once state");
  requireIncludes(recorder, "database_url_retained: false", "recorder database URL state");

  console.log(
    `[verify-pages-cache-consumer-execution] PASS source_packets=${packets.length} tests=${testCommands.length} target=/consumers/0/cache_consumer/executed_evidence`,
  );
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
