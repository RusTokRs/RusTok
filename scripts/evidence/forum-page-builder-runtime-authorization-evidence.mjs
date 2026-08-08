#!/usr/bin/env node

import { spawnSync } from "node:child_process";
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
const contractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/evidence/forum-page-builder-runtime-authorization-execution-contract.json",
);
const MAX_CAPTURE_BYTES = 8 * 1024 * 1024;
const MAX_COMMANDS = 16;
const MAX_ARG_LENGTH = 4096;

function fail(message) {
  throw new Error(`Forum Page Builder runtime authorization evidence failed: ${message}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function regularFileRecord(relativePath, label) {
  if (
    typeof relativePath !== "string" ||
    relativePath.length === 0 ||
    relativePath.length > 4096 ||
    relativePath.includes("\0")
  ) {
    fail(`${label} has an invalid path`);
  }
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail(`${label} must remain inside the repository`);
  }
  if (!existsSync(absolute)) fail(`${label} is missing`);
  const link = lstatSync(absolute);
  if (link.isSymbolicLink() || !link.isFile()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const stats = statSync(absolute);
  if (stats.size <= 0 || stats.size > MAX_CAPTURE_BYTES) {
    fail(`${label} must be a bounded non-empty file`);
  }
  const bytes = readFileSync(absolute);
  return { bytes: stats.size, sha256: sha256(bytes) };
}

function outputPath(contract) {
  const environmentName = contract.output?.environment;
  const defaultPath = contract.output?.default_path;
  if (typeof environmentName !== "string" || typeof defaultPath !== "string") {
    fail("contract output path is invalid");
  }
  const raw = process.env[environmentName];
  if (
    raw !== undefined &&
    (raw.length === 0 || raw.length > 16_384 || /[\u0000\r\n]/u.test(raw))
  ) {
    fail(`${environmentName} is outside the bounded output input`);
  }
  const requested = raw ?? defaultPath;
  const absolute = path.isAbsolute(requested)
    ? path.resolve(requested)
    : path.resolve(repoRoot, requested);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("runtime evidence output must remain inside repository target/");
  }
  return absolute;
}

function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
    shell: false,
    maxBuffer: 1024 * 1024,
  });
  if (result.error) fail(`git HEAD lookup failed: ${result.error.message}`);
  if (result.status !== 0) fail("git HEAD lookup returned a non-zero status");
  const value = result.stdout.trim();
  if (!/^[0-9a-f]{40}$/u.test(value)) fail("git HEAD is not a full lowercase SHA");
  return value;
}

function sourceHashes(contract) {
  if (
    !Array.isArray(contract.required_source_files) ||
    contract.required_source_files.length === 0 ||
    contract.required_source_files.length > 128
  ) {
    fail("required source-file set is outside the bounded contract");
  }
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => [
      relativePath,
      regularFileRecord(relativePath, `source file ${relativePath}`).sha256,
    ]),
  );
}

function validatedCommands(contract) {
  if (
    !Array.isArray(contract.commands) ||
    contract.commands.length === 0 ||
    contract.commands.length > MAX_COMMANDS
  ) {
    fail("runtime command set is outside the bounded contract");
  }
  const seen = new Set();
  return contract.commands.map((command) => {
    if (
      command === null ||
      typeof command !== "object" ||
      typeof command.id !== "string" ||
      !/^[a-z0-9_]{1,64}$/u.test(command.id) ||
      seen.has(command.id)
    ) {
      fail("runtime command id is invalid or duplicated");
    }
    seen.add(command.id);
    if (command.program !== "cargo") {
      fail(`runtime command ${command.id} must use the allowlisted cargo program`);
    }
    if (
      !Array.isArray(command.args) ||
      command.args.length === 0 ||
      command.args.length > 32 ||
      command.args.some(
        (arg) =>
          typeof arg !== "string" ||
          arg.length === 0 ||
          arg.length > MAX_ARG_LENGTH ||
          /[\u0000\r\n]/u.test(arg),
      )
    ) {
      fail(`runtime command ${command.id} has invalid argv`);
    }
    if (command.args[0] !== "test") {
      fail(`runtime command ${command.id} must remain a cargo test command`);
    }
    return {
      id: command.id,
      program: command.program,
      args: [...command.args],
    };
  });
}

function boundedCapture(value, label) {
  const buffer = Buffer.isBuffer(value) ? value : Buffer.from(value ?? "");
  if (buffer.byteLength > MAX_CAPTURE_BYTES) {
    fail(`${label} exceeds ${MAX_CAPTURE_BYTES} bytes`);
  }
  return {
    bytes: buffer.byteLength,
    sha256: sha256(buffer),
  };
}

function executeCommand(command) {
  const result = spawnSync(command.program, command.args, {
    cwd: repoRoot,
    shell: false,
    encoding: null,
    maxBuffer: MAX_CAPTURE_BYTES,
    env: process.env,
  });
  if (result.error) {
    fail(`runtime command ${command.id} could not execute: ${result.error.message}`);
  }
  const stdout = boundedCapture(result.stdout, `${command.id} stdout`);
  const stderr = boundedCapture(result.stderr, `${command.id} stderr`);
  const status = result.status;
  if (!Number.isInteger(status)) {
    fail(`runtime command ${command.id} did not return an integer status`);
  }
  const observation = {
    id: command.id,
    program: command.program,
    args: command.args,
    status,
    stdout,
    stderr,
  };
  if (status !== 0) {
    const error = new Error(`runtime command ${command.id} failed with status ${status}`);
    error.observation = observation;
    throw error;
  }
  return observation;
}

function writeAtomic(location, document) {
  mkdirSync(path.dirname(location), { recursive: true });
  const temporary = `${location}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, location);
}

function main() {
  const contract = JSON.parse(readFileSync(contractPath, "utf8"));
  if (contract.status !== "source_ready_maintainer_execution_pending") {
    fail("runtime evidence contract must not claim execution before the runner starts");
  }
  if (
    contract.output?.format !== "forum_page_builder_runtime_authorization_execution_v1" ||
    contract.output?.status !== "runtime_authorization_execution_passed_wave_pending"
  ) {
    fail("runtime evidence output identity drifted");
  }

  const output = outputPath(contract);
  rmSync(output, { force: true });
  const sourceCommit = currentCommit();
  const sources = sourceHashes(contract);
  const commands = validatedCommands(contract);
  const observations = [];

  for (const command of commands) {
    observations.push(executeCommand(command));
  }

  writeAtomic(output, {
    format: contract.output.format,
    status: contract.output.status,
    source_commit: sourceCommit,
    source_files: sources,
    commands: observations,
    retained_raw_command_output: false,
    runtime_authorization_execution_only: true,
    deployed_server_fn_attestation_pending: true,
    browser_execution_pending: true,
    provider_slo_health_unobserved: true,
    observed_page_builder_wave_pending: true,
    executed_at: new Date().toISOString(),
  });
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
