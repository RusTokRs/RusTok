#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = "crates/rustok-pages/contracts/evidence/pages-cache-consumer-execution.json";
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;
const INTEGER_PATTERN = /^[1-9][0-9]*$/u;

function fail(message) {
  throw new Error(`Pages cache-consumer execution recorder failed: ${message}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function regularFile(location, label, maximumBytes = MAX_SOURCE_BYTES) {
  if (!existsSync(location)) fail(`${label} is missing`);
  const metadata = lstatSync(location);
  if (metadata.isSymbolicLink() || !metadata.isFile()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const size = statSync(location).size;
  if (size <= 0 || size > maximumBytes) fail(`${label} is outside the bounded size`);
  const bytes = readFileSync(location);
  return { bytes, size, sha256: sha256(bytes) };
}

function repoFile(relativePath, label) {
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) fail(`${label} escapes repository root`);
  return regularFile(absolute, label);
}

function repoJson(relativePath, label) {
  const record = repoFile(relativePath, label);
  let document;
  try {
    document = JSON.parse(record.bytes.toString("utf8"));
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
  if (document === null || typeof document !== "object" || Array.isArray(document)) {
    fail(`${label} must be a JSON object`);
  }
  return { document, ...record };
}

function currentCommit() {
  const value = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
  if (!COMMIT_PATTERN.test(value)) fail("checkout HEAD is not a full lowercase Git SHA");
  return value;
}

function requiredEnv(name, pattern) {
  const value = process.env[name];
  if (typeof value !== "string" || value.length === 0 || value.length > 1024) {
    fail(`${name} is missing or unbounded`);
  }
  if (pattern && !pattern.test(value)) fail(`${name} is invalid`);
  return value;
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

function outputPath(contract, argv) {
  if (argv.length > 2) {
    fail("usage: record-pages-cache-consumer-execution.mjs [--output FILE]");
  }
  let candidate = contract.output?.default_path;
  if (argv.length > 0) {
    if (argv[0] !== "--output" || argv.length !== 2) {
      fail("usage: record-pages-cache-consumer-execution.mjs [--output FILE]");
    }
    candidate = argv[1];
  }
  if (
    typeof candidate !== "string" ||
    candidate.length === 0 ||
    candidate.length > 16_384 ||
    /[\u0000\r\n]/u.test(candidate)
  ) {
    fail("output path is invalid");
  }
  const absolute = path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
  const evidenceRoot = path.resolve(repoRoot, "evidence/pages-cache-consumer");
  const relative = path.relative(evidenceRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("output must remain inside evidence/pages-cache-consumer/");
  }
  return absolute;
}

function writeAtomic(location, document) {
  mkdirSync(path.dirname(location), { recursive: true });
  const temporary = `${location}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, location);
}

function main() {
  if (process.env.GITHUB_ACTIONS !== "true") {
    fail("GITHUB_ACTIONS=true is required; local execution cannot mint an execution receipt");
  }

  const contractRecord = repoJson(contractPath, "cache consumer execution contract");
  const contract = contractRecord.document;
  if (
    contract.format !== "pages_cache_consumer_execution_source_v1" ||
    contract.status !== "source_ready_main_execution_pending"
  ) {
    fail("cache consumer execution contract identity drifted");
  }

  const head = currentCommit();
  const githubSha = requiredEnv("GITHUB_SHA", COMMIT_PATTERN);
  if (githubSha !== head) fail("GITHUB_SHA does not equal checkout HEAD");
  const workflow = requiredEnv("GITHUB_WORKFLOW");
  const runId = requiredEnv("GITHUB_RUN_ID", INTEGER_PATTERN);
  const runAttempt = requiredEnv("GITHUB_RUN_ATTEMPT", INTEGER_PATTERN);
  const eventName = requiredEnv("GITHUB_EVENT_NAME");
  const repository = requiredEnv("GITHUB_REPOSITORY");
  const refName = requiredEnv("GITHUB_REF_NAME");
  if (repository !== "RusTokRs/RusTok") fail("execution repository is not canonical");
  if (workflow !== "Pages Cache Consumer Execution Evidence") fail("execution workflow identity drifted");
  if (eventName !== "push" || refName !== "main") {
    fail("only an exact main push may mint a cache consumer execution receipt");
  }

  const registry = repoJson(contract.target.path, "Page Builder FBA registry");
  if (registry.document.status !== contract.target.required_registry_status) {
    fail("Page Builder FBA registry status drifted before execution");
  }
  const targetBefore = pointerValue(registry.document, contract.target.executed_evidence_json_pointer);
  if (targetBefore !== contract.target.required_before_value) {
    fail("cache consumer executed_evidence is no longer pending");
  }

  for (const packet of Object.values(contract.source_packets ?? {})) {
    const record = repoJson(packet.path, `source packet ${packet.path}`);
    if (record.document.status !== packet.required_status) {
      fail(`source packet status drifted for ${packet.path}`);
    }
  }

  if (!Array.isArray(contract.execution?.verifier_commands) || contract.execution.verifier_commands.length < 8) {
    fail("execution verifier_commands are incomplete");
  }
  if (!Array.isArray(contract.execution?.test_commands) || contract.execution.test_commands.length < 8) {
    fail("execution test_commands are incomplete");
  }
  if (!Array.isArray(contract.execution?.check_commands) || contract.execution.check_commands.length < 3) {
    fail("execution check_commands are incomplete");
  }
  if (contract.execution?.postgres?.required !== true) fail("PostgreSQL execution must remain required");
  if (!Array.isArray(contract.required_source_files) || contract.required_source_files.length === 0) {
    fail("required_source_files must be non-empty");
  }

  const sourceSha256 = Object.fromEntries(
    contract.required_source_files.map((relativePath) => [
      relativePath,
      repoFile(relativePath, `required source ${relativePath}`).sha256,
    ]),
  );

  const receipt = {
    format: contract.output.format,
    status: contract.output.success_status,
    generated_at: new Date().toISOString(),
    source_commit: head,
    provenance: {
      repository,
      workflow,
      run_id: runId,
      run_attempt: runAttempt,
      event_name: eventName,
      head_branch: refName,
      github_actions: true,
      cryptographic_ci_attestation_claimed: false,
    },
    target: {
      path: contract.target.path,
      registry_status_before: registry.document.status,
      registry_sha256: registry.sha256,
      json_pointer: contract.target.executed_evidence_json_pointer,
      before: targetBefore,
    },
    execution: {
      verifier_commands: contract.execution.verifier_commands,
      test_commands: contract.execution.test_commands,
      check_commands: contract.execution.check_commands,
      all_commands_passed: true,
      packet_generated_only_after_verifier_test_and_check_steps: true,
      postgres_used: true,
      isolated_postgres_schemas_owned_by_harnesses: true,
      sqlite_used: true,
      registered_native_route_executed: true,
      artifact_http_route_executed: true,
      production_generation_gate_executed: true,
      durable_outbox_retry_executed: true,
      cache_miss_refill_hit_executed: true,
      same_event_retry_and_async_listener_dedupe_executed: true,
      browser_used: false,
      external_deployment_used: false,
      cross_process_exact_once_claimed: false,
    },
    source_sha256: sourceSha256,
    governance: {
      fba_registry_mutated: false,
      executed_evidence_cleared: false,
      receipt_is_admission_input_only: true,
      separate_registry_admission_required: true,
      terminal_inventory_complete_claimed: false,
      owner_approval_claimed: false,
      platform_approval_claimed: false,
      pages_ffa_promoted: false,
      page_builder_fba_promoted: false,
    },
    privacy: {
      database_url_retained: false,
      raw_test_logs_embedded: false,
      tenant_identity_retained: false,
      credentials_or_cookies_retained: false,
      raw_http_payload_retained: false,
    },
  };

  const output = outputPath(contract, process.argv.slice(2));
  writeAtomic(output, receipt);
  console.log(
    `[record-pages-cache-consumer-execution] PASS source=${head} run_id=${runId} cache_consumer=passed admission=pending`,
  );
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
