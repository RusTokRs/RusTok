#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy-connector/contracts/evidence/consumer-poison-postgres-execution-contract.json";
const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const outputPath = resolve(repoRoot, contract.evidence_path);

function fail(message) {
  throw new Error(message);
}

function run(program, args, env = process.env) {
  const result = spawnSync(program, args, {
    cwd: repoRoot,
    env,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.error) {
    fail(`${program} could not start: ${result.error.message}`);
  }
  return {
    status: result.status ?? -1,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

function runChecked(program, args, env = process.env) {
  const result = run(program, args, env);
  if (result.status !== 0) {
    fail(`${program} exited with status ${result.status}; no retained evidence was written`);
  }
  return result;
}

function oneLine(value, field) {
  const line = value.trim().split(/\r?\n/, 1)[0]?.trim() ?? "";
  if (!line || line.length > 256 || /[\u0000-\u001f\u007f]/u.test(line)) {
    fail(`${field} is missing or outside the retained evidence boundary`);
  }
  return line;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function fileSha256(relativePath) {
  return sha256(readFileSync(resolve(repoRoot, relativePath)));
}

function sourceHashes() {
  return Object.fromEntries(
    contract.source_files.map((relativePath) => [relativePath, fileSha256(relativePath)]),
  );
}

function sameRecord(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function requirePassedCase(output, caseName) {
  const pattern = new RegExp(
    `(?:^|\\r?\\n)test ${escapeRegExp(caseName)} \\.\\.\\. ok(?:\\r?\\n|$)`,
    "u",
  );
  if (!pattern.test(output)) {
    fail(`required PostgreSQL case did not report success: ${caseName}`);
  }
}

function markerValues(output, marker) {
  const prefix = `RUSTOK_IGGY_POISON_EVIDENCE ${marker}=`;
  return output
    .split(/\r?\n/u)
    .filter((line) => line.startsWith(prefix))
    .map((line) => line.slice(prefix.length).trim())
    .filter(Boolean);
}

function requireOneMarker(output, marker) {
  const values = [...new Set(markerValues(output, marker))];
  if (values.length !== 1) {
    fail(`expected exactly one retained evidence marker for ${marker}`);
  }
  return oneLine(values[0], marker);
}

function ensureOutputInsideRepository() {
  const root = resolve(repoRoot) + sep;
  if (!outputPath.startsWith(root)) {
    fail("retained evidence output path must stay inside the repository");
  }
}

function validateDatabaseUrl() {
  const primary = process.env[contract.database_url_env];
  const fallback = process.env[contract.database_url_fallback_env];
  const value = primary || fallback;
  if (!value) {
    fail(
      `${contract.database_url_env} or ${contract.database_url_fallback_env} must provide an opt-in PostgreSQL URL`,
    );
  }
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail("the opt-in PostgreSQL URL is invalid");
  }
  if (parsed.protocol !== "postgres:" && parsed.protocol !== "postgresql:") {
    fail("the retained evidence runner accepts PostgreSQL URLs only");
  }
  return primary ? contract.database_url_env : contract.database_url_fallback_env;
}

function ensureCleanCommit() {
  const status = runChecked("git", ["status", "--porcelain=v1", "--untracked-files=all"]);
  if (status.stdout.trim()) {
    fail("working tree must be clean before retained evidence execution");
  }
  const commit = oneLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout,
    "git_commit",
  );
  if (!/^[0-9a-f]{40}$/u.test(commit)) {
    fail("git commit must be a full lowercase SHA-1");
  }
  return commit;
}

function executeContractCommand(command) {
  return runChecked(command.program, command.args);
}

function writeAtomically(packet) {
  ensureOutputInsideRepository();
  mkdirSync(dirname(outputPath), { recursive: true });
  const temporaryPath = `${outputPath}.tmp-${process.pid}`;
  if (existsSync(temporaryPath)) {
    unlinkSync(temporaryPath);
  }
  writeFileSync(temporaryPath, `${JSON.stringify(packet, null, 2)}\n`, "utf8");
  renameSync(temporaryPath, outputPath);
}

try {
  ensureOutputInsideRepository();
  const databaseUrlSource = validateDatabaseUrl();
  const gitCommit = ensureCleanCommit();
  const initialSourceSha256 = sourceHashes();
  const cargoVersion = oneLine(runChecked("cargo", ["--version"]).stdout, "cargo_version");
  const rustcVersion = oneLine(runChecked("rustc", ["--version"]).stdout, "rustc_version");
  const startedAt = new Date().toISOString();

  if (!Array.isArray(contract.commands) || contract.commands.length !== 2) {
    fail("retained evidence contract must contain exactly two commands");
  }
  const environmentResult = executeContractCommand(contract.commands[0]);
  const scenarioResult = executeContractCommand(contract.commands[1]);
  const environmentOutput = `${environmentResult.stdout}\n${environmentResult.stderr}`;
  const scenarioOutput = `${scenarioResult.stdout}\n${scenarioResult.stderr}`;

  const postgresServerVersion = requireOneMarker(
    environmentOutput,
    "postgres_server_version",
  );
  const postgresServerVersionNum = requireOneMarker(
    environmentOutput,
    "postgres_server_version_num",
  );
  if (!/^\d+$/u.test(postgresServerVersionNum)) {
    fail("postgres_server_version_num must contain digits only");
  }

  for (const requiredCase of contract.required_cases) {
    requirePassedCase(scenarioOutput, requiredCase.case);
  }
  if (scenarioOutput.includes("skipping consumer poison receipt PostgreSQL evidence")) {
    fail("scenario tests reported a skip instead of PostgreSQL execution");
  }

  const finalCommit = oneLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout,
    "final_git_commit",
  );
  if (finalCommit !== gitCommit) {
    fail("git commit changed during retained evidence execution");
  }
  const finalSourceSha256 = sourceHashes();
  if (!sameRecord(finalSourceSha256, initialSourceSha256)) {
    fail("retained evidence source files changed during execution");
  }
  const completedAt = new Date().toISOString();
  const combinedOutput = `${environmentOutput}\n--- consumer poison scenarios ---\n${scenarioOutput}`;

  const packet = {
    schema_version: 1,
    module: contract.module,
    packet: "consumer-poison-postgres-runtime-evidence",
    status: "postgres_runtime_executed",
    generated_from: contractPath,
    runner: contract.runner,
    verifier: contract.verifier,
    git_commit: gitCommit,
    working_tree_clean_before_run: true,
    started_at: startedAt,
    completed_at: completedAt,
    database_url_source: databaseUrlSource,
    database: {
      backend: "postgresql",
      server_version: postgresServerVersion,
      server_version_num: postgresServerVersionNum,
    },
    toolchain: {
      cargo: cargoVersion,
      rustc: rustcVersion,
    },
    commands: contract.commands,
    source_sha256: finalSourceSha256,
    test_output_sha256: sha256(combinedOutput),
    test_output_bytes: Buffer.byteLength(combinedOutput),
    executed_cases: contract.required_cases.map((requiredCase) => ({
      case: requiredCase.case,
      result: "pass",
      assertions: requiredCase.assertions,
    })),
  };

  writeAtomically(packet);
  console.log(`Retained PostgreSQL evidence written to ${contract.evidence_path}`);
} catch (error) {
  console.error(`PostgreSQL poison receipt evidence capture failed: ${error.message}`);
  process.exit(1);
}
