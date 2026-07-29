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
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-product/contracts/evidence/product-catalog-separate-process-runtime-contract.json";
const expectedRunnerPath =
  "scripts/evidence/capture-product-catalog-separate-process-runtime.mjs";
const expectedVerifierPath =
  "scripts/verify/verify-product-catalog-separate-process-runtime-contract.mjs";
const expectedEvidencePath =
  "crates/rustok-product/contracts/evidence/product-catalog-separate-process-runtime.json";
const expectedCommands = {
  provider: {
    program: "cargo",
    args: ["run", "-p", "rustok-product-catalog-service"],
  },
  probe: {
    program: "cargo",
    args: [
      "run",
      "-p",
      "rustok-product-transport",
      "--example",
      "product_catalog_runtime_probe",
    ],
  },
  consumer: {
    program: "cargo",
    args: [
      "run",
      "-p",
      "rustok-server",
      "--no-default-features",
      "--features",
      "mod-commerce,mod-ai",
    ],
  },
};
const expectedRequiredEnvironment = [
  "RUSTOK_PRODUCT_CATALOG_DATABASE_URL",
  "RUSTOK_PRODUCT_CATALOG_EVIDENCE_CONSUMER_DATABASE_URL",
  "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN",
  "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR",
  "RUSTOK_PRODUCT_CATALOG_SERVICE_BIND",
  "RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT",
  "RUSTOK_PRODUCT_CATALOG_EVIDENCE_TENANT_ID",
  "RUSTOK_PRODUCT_CATALOG_EVIDENCE_PRODUCT_ID",
  "RUSTOK_PRODUCT_CATALOG_EVIDENCE_VARIANT_ID",
];
const expectedForcedEnvironment = {
  RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK: "true",
  RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK: "true",
  RUSTOK_PRODUCT_CATALOG_PROVIDER: "grpc",
  RUSTOK_ENV: "development",
  RUSTOK_LOG_FORMAT: "json",
  OTEL_ENABLED: "false",
};
const expectedMarkers = {
  provider: [
    "Product catalog database schema preflight passed",
    "Product catalog gRPC service listening",
  ],
  probe: [
    "PRODUCT_CATALOG_RUNTIME_PROBE_OK operations=3 product_projection=matched variant_projection=matched published_list=nonempty",
  ],
  consumer: [
    "Product catalog deployment provider initialized",
    "RusTok Axum host listening",
  ],
};
const maximumCapturedOutputBytes = 32 * 1024 * 1024;
const defaultStartupTimeoutMs = 180_000;
const defaultShutdownTimeoutMs = 15_000;
const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const outputPath = resolve(repoRoot, contract.evidence_path);

function fail(message) {
  throw new Error(message);
}

function sameValue(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
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

function oneLine(value, field, maximumLength = 4096) {
  if (typeof value !== "string" || value.trim() !== value || !value) {
    fail(`${field} must be a non-empty value without surrounding whitespace`);
  }
  if (value.length > maximumLength || /[\u0000-\u001f\u007f]/u.test(value)) {
    fail(`${field} is outside the runtime evidence boundary`);
  }
  return value;
}

function requiredEnvironment(name, maximumLength = 4096) {
  return oneLine(process.env[name] ?? "", name, maximumLength);
}

function optionalEnvironment(name, maximumLength = 4096) {
  const value = process.env[name];
  if (value === undefined || value === "") return null;
  return oneLine(value, name, maximumLength);
}

function boundedIntegerEnvironment(name, fallback, minimum, maximum) {
  const value = optionalEnvironment(name, 16);
  if (value === null) return fallback;
  if (!/^\d+$/u.test(value)) fail(`${name} must be an integer`);
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < minimum || parsed > maximum) {
    fail(`${name} must be between ${minimum} and ${maximum}`);
  }
  return parsed;
}

function validateUuid(value, field) {
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu.test(
      value,
    )
  ) {
    fail(`${field} must be a UUID`);
  }
  return value;
}

function validatePostgresUrl(value, field) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail(`${field} must be an absolute PostgreSQL URL`);
  }
  if (
    !["postgres:", "postgresql:"].includes(parsed.protocol) ||
    !parsed.hostname ||
    parsed.pathname === "/" ||
    parsed.hash
  ) {
    fail(`${field} must identify a PostgreSQL database without a fragment`);
  }
  return value;
}

function validateLoopbackBinding(value) {
  const match = /^(127\.0\.0\.1|\[::1\]):(\d+)$/u.exec(value);
  if (!match) {
    fail(
      "RUSTOK_PRODUCT_CATALOG_SERVICE_BIND must be an explicit loopback IP and port for retained local evidence",
    );
  }
  const port = Number(match[2]);
  if (!Number.isInteger(port) || port < 1 || port > 65_535) {
    fail("RUSTOK_PRODUCT_CATALOG_SERVICE_BIND port is invalid");
  }
  return { host: match[1], port };
}

function validateLoopbackEndpoint(value, binding) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail("RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT must be an absolute URL");
  }
  if (
    parsed.protocol !== "http:" ||
    parsed.username ||
    parsed.password ||
    parsed.search ||
    parsed.hash ||
    !["", "/"].includes(parsed.pathname)
  ) {
    fail(
      "retained local Product evidence requires an HTTP loopback endpoint without credentials, path, query, or fragment",
    );
  }
  const endpointHost = parsed.hostname.toLowerCase();
  const bindingHost = binding.host === "[::1]" ? "[::1]" : binding.host;
  if (![
    "127.0.0.1",
    "[::1]",
    "::1",
  ].includes(endpointHost)) {
    fail("RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT must use a loopback host");
  }
  const endpointPort = Number(parsed.port || "80");
  if (endpointPort !== binding.port) {
    fail("Product provider bind and consumer gRPC endpoint ports must match");
  }
  if (bindingHost === "127.0.0.1" && endpointHost !== "127.0.0.1") {
    fail("Product provider bind and consumer gRPC endpoint hosts must match");
  }
  if (bindingHost === "[::1]" && !["[::1]", "::1"].includes(endpointHost)) {
    fail("Product provider bind and consumer gRPC endpoint hosts must match");
  }
  return value;
}

function validateNoTlsOverrides() {
  for (const name of [
    "RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_CERT_PATH",
    "RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_KEY_PATH",
    "RUSTOK_PRODUCT_CATALOG_GRPC_TLS_DOMAIN",
  ]) {
    if (optionalEnvironment(name) !== null) {
      fail(`${name} must be unset for the locked local-loopback evidence profile`);
    }
  }
}

function validateContractBoundary() {
  if (
    contract.schema_version !== 1 ||
    contract.module !== "product" ||
    contract.packet !== "product-catalog-separate-process-runtime-contract" ||
    contract.status !== "runtime_execution_contract_locked"
  ) {
    fail("Product separate-process runtime contract identity drift");
  }
  if (
    contract.runner !== expectedRunnerPath ||
    contract.verifier !== expectedVerifierPath ||
    contract.evidence_path !== expectedEvidencePath ||
    contract.evidence_status !== "runtime_execution_pending"
  ) {
    fail("Product separate-process runtime tooling boundary drift");
  }
  if (!sameValue(contract.commands, expectedCommands)) {
    fail("Product separate-process command allowlist drift");
  }
  if (!sameValue(contract.required_environment, expectedRequiredEnvironment)) {
    fail("Product separate-process required environment allowlist drift");
  }
  if (!sameValue(contract.forced_environment, expectedForcedEnvironment)) {
    fail("Product separate-process forced environment drift");
  }
  if (!sameValue(contract.readiness_markers, expectedMarkers)) {
    fail("Product separate-process readiness markers drift");
  }
  if (
    contract.execution_scope !==
      "provider_schema_preflight_authenticated_rpc_and_consumer_remote_startup" ||
    contract.promotion_gate !==
      "does_not_close_commerce_or_ai_business_end_to_end_without_separate_requests"
  ) {
    fail("Product separate-process promotion boundary drift");
  }
}

function run(program, args, env = process.env) {
  const result = spawnSync(program, args, {
    cwd: repoRoot,
    env,
    encoding: "utf8",
    maxBuffer: maximumCapturedOutputBytes,
  });
  if (result.error) fail(`${program} could not start`);
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

function versionLine(program, args, field) {
  const value = runChecked(program, args).stdout.trim().split(/\r?\n/u, 1)[0] ?? "";
  return oneLine(value, field, 256);
}

function workingTreeStatus() {
  return runChecked("git", ["status", "--porcelain=v1", "--untracked-files=all"]).stdout;
}

function ensureCleanCommit() {
  if (workingTreeStatus().trim()) {
    fail("working tree must be clean before retained Product runtime execution");
  }
  const commit = oneLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout.trim(),
    "git_commit",
    40,
  );
  if (!/^[0-9a-f]{40}$/u.test(commit)) {
    fail("git commit must be a full lowercase SHA-1");
  }
  return commit;
}

function ensureOutputInsideRepository() {
  const prefix = resolve(repoRoot) + sep;
  if (!outputPath.startsWith(prefix)) {
    fail("retained Product runtime evidence path must stay inside the repository");
  }
}

function writeAtomically(packet) {
  ensureOutputInsideRepository();
  mkdirSync(dirname(outputPath), { recursive: true });
  const temporaryPath = `${outputPath}.tmp-${process.pid}`;
  if (existsSync(temporaryPath)) unlinkSync(temporaryPath);
  writeFileSync(temporaryPath, `${JSON.stringify(packet, null, 2)}\n`, "utf8");
  renameSync(temporaryPath, outputPath);
}

function startProcess(command, env, label) {
  const child = spawn(command.program, command.args, {
    cwd: repoRoot,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  let exited = false;
  let exitCode = null;
  let exitSignal = null;
  let processError = null;
  const watchers = new Set();

  const notify = () => {
    for (const watcher of watchers) watcher();
  };
  const append = (chunk) => {
    output += chunk.toString("utf8");
    if (Buffer.byteLength(output) > maximumCapturedOutputBytes) {
      processError = new Error(`${label} output exceeded the retained evidence boundary`);
      child.kill("SIGKILL");
    }
    notify();
  };
  child.stdout.on("data", append);
  child.stderr.on("data", append);
  child.on("error", () => {
    processError = new Error(`${label} could not start`);
    notify();
  });
  const exitPromise = new Promise((resolveExit) => {
    child.on("exit", (code, signal) => {
      exited = true;
      exitCode = code;
      exitSignal = signal;
      notify();
      resolveExit();
    });
  });

  return {
    child,
    label,
    exitPromise,
    get output() {
      return output;
    },
    get exited() {
      return exited;
    },
    get exitCode() {
      return exitCode;
    },
    get exitSignal() {
      return exitSignal;
    },
    async waitForMarkers(markers, timeoutMs) {
      await new Promise((resolveReady, rejectReady) => {
        let timer;
        const check = () => {
          if (processError) {
            cleanup();
            rejectReady(processError);
            return;
          }
          if (markers.every((marker) => output.includes(marker))) {
            cleanup();
            resolveReady();
            return;
          }
          if (exited) {
            cleanup();
            rejectReady(new Error(`${label} exited before all readiness markers were observed`));
          }
        };
        const cleanup = () => {
          clearTimeout(timer);
          watchers.delete(check);
        };
        timer = setTimeout(() => {
          cleanup();
          rejectReady(new Error(`${label} readiness timed out`));
        }, timeoutMs);
        watchers.add(check);
        check();
      });
    },
  };
}

async function stopProcess(record, shutdownTimeoutMs) {
  if (!record || record.exited) return;
  record.child.kill("SIGTERM");
  await Promise.race([
    record.exitPromise,
    new Promise((resolveDelay) => setTimeout(resolveDelay, shutdownTimeoutMs)),
  ]);
  if (!record.exited) {
    record.child.kill("SIGKILL");
    await record.exitPromise;
  }
}

function outputMetadata(output) {
  return {
    sha256: sha256(output),
    bytes: Buffer.byteLength(output),
  };
}

function rejectSecretLeaks(outputs, secrets) {
  for (const output of outputs) {
    for (const secret of secrets) {
      if (secret && output.includes(secret)) {
        fail("captured Product runtime output contained a forbidden secret; no evidence was written");
      }
    }
  }
}

function childEnvironment(base, additions) {
  const environment = { ...base, ...additions };
  delete environment.RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_CERT_PATH;
  delete environment.RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_KEY_PATH;
  delete environment.RUSTOK_PRODUCT_CATALOG_GRPC_TLS_DOMAIN;
  return environment;
}

let providerProcess;
let consumerProcess;
try {
  ensureOutputInsideRepository();
  validateContractBoundary();
  validateNoTlsOverrides();

  const providerDatabaseUrl = validatePostgresUrl(
    requiredEnvironment("RUSTOK_PRODUCT_CATALOG_DATABASE_URL"),
    "RUSTOK_PRODUCT_CATALOG_DATABASE_URL",
  );
  const consumerDatabaseUrl = validatePostgresUrl(
    requiredEnvironment("RUSTOK_PRODUCT_CATALOG_EVIDENCE_CONSUMER_DATABASE_URL"),
    "RUSTOK_PRODUCT_CATALOG_EVIDENCE_CONSUMER_DATABASE_URL",
  );
  const bearerToken = requiredEnvironment("RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN", 4096);
  const trustedActor = requiredEnvironment(
    "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR",
    128,
  );
  const binding = validateLoopbackBinding(
    requiredEnvironment("RUSTOK_PRODUCT_CATALOG_SERVICE_BIND", 64),
  );
  const endpoint = validateLoopbackEndpoint(
    requiredEnvironment("RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT", 256),
    binding,
  );
  const tenantId = validateUuid(
    requiredEnvironment("RUSTOK_PRODUCT_CATALOG_EVIDENCE_TENANT_ID", 36),
    "RUSTOK_PRODUCT_CATALOG_EVIDENCE_TENANT_ID",
  );
  const productId = validateUuid(
    requiredEnvironment("RUSTOK_PRODUCT_CATALOG_EVIDENCE_PRODUCT_ID", 36),
    "RUSTOK_PRODUCT_CATALOG_EVIDENCE_PRODUCT_ID",
  );
  const variantId = validateUuid(
    requiredEnvironment("RUSTOK_PRODUCT_CATALOG_EVIDENCE_VARIANT_ID", 36),
    "RUSTOK_PRODUCT_CATALOG_EVIDENCE_VARIANT_ID",
  );
  const startupTimeoutMs = boundedIntegerEnvironment(
    "RUSTOK_PRODUCT_CATALOG_EVIDENCE_STARTUP_TIMEOUT_MS",
    defaultStartupTimeoutMs,
    1_000,
    600_000,
  );
  const shutdownTimeoutMs = boundedIntegerEnvironment(
    "RUSTOK_PRODUCT_CATALOG_EVIDENCE_SHUTDOWN_TIMEOUT_MS",
    defaultShutdownTimeoutMs,
    1_000,
    60_000,
  );

  const gitCommit = ensureCleanCommit();
  const initialSourceSha256 = sourceHashes();
  const cargoVersion = versionLine("cargo", ["--version"], "cargo_version");
  const rustcVersion = versionLine("rustc", ["--version"], "rustc_version");
  const startedAt = new Date().toISOString();
  const baseEnvironment = { ...process.env };

  const providerEnvironment = childEnvironment(baseEnvironment, {
    RUSTOK_PRODUCT_CATALOG_DATABASE_URL: providerDatabaseUrl,
    RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN: bearerToken,
    RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR: trustedActor,
    RUSTOK_PRODUCT_CATALOG_SERVICE_BIND: `${binding.host}:${binding.port}`,
    RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK: "true",
    RUSTOK_LOG_FORMAT: "json",
    RUSTOK_METRICS: "false",
    OTEL_ENABLED: "false",
  });
  providerProcess = startProcess(
    contract.commands.provider,
    providerEnvironment,
    "Product catalog provider",
  );
  await providerProcess.waitForMarkers(contract.readiness_markers.provider, startupTimeoutMs);

  const probeEnvironment = childEnvironment(baseEnvironment, {
    RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT: endpoint,
    RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN: bearerToken,
    RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK: "true",
    RUSTOK_PRODUCT_CATALOG_EVIDENCE_TENANT_ID: tenantId,
    RUSTOK_PRODUCT_CATALOG_EVIDENCE_PRODUCT_ID: productId,
    RUSTOK_PRODUCT_CATALOG_EVIDENCE_VARIANT_ID: variantId,
  });
  for (const optionalName of contract.optional_environment) {
    const value = optionalEnvironment(optionalName);
    if (value !== null) probeEnvironment[optionalName] = value;
  }
  const probeResult = runChecked(
    contract.commands.probe.program,
    contract.commands.probe.args,
    probeEnvironment,
  );
  const probeOutput = `${probeResult.stdout}\n${probeResult.stderr}`;
  for (const marker of contract.readiness_markers.probe) {
    if (!probeOutput.includes(marker)) {
      fail("authenticated Product catalog runtime probe did not report its success marker");
    }
  }

  const consumerEnvironment = childEnvironment(baseEnvironment, {
    DATABASE_URL: consumerDatabaseUrl,
    RUSTOK_PRODUCT_CATALOG_PROVIDER: "grpc",
    RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT: endpoint,
    RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN: bearerToken,
    RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK: "true",
    RUSTOK_ENV: "development",
    RUSTOK_LOG_FORMAT: "json",
    RUSTOK_METRICS: "false",
    OTEL_ENABLED: "false",
  });
  consumerProcess = startProcess(
    contract.commands.consumer,
    consumerEnvironment,
    "RusTok consumer server",
  );
  await consumerProcess.waitForMarkers(contract.readiness_markers.consumer, startupTimeoutMs);

  await stopProcess(consumerProcess, shutdownTimeoutMs);
  await stopProcess(providerProcess, shutdownTimeoutMs);

  rejectSecretLeaks(
    [providerProcess.output, probeOutput, consumerProcess.output],
    [providerDatabaseUrl, consumerDatabaseUrl, bearerToken],
  );
  const finalCommit = oneLine(
    runChecked("git", ["rev-parse", "HEAD"]).stdout.trim(),
    "final_git_commit",
    40,
  );
  if (finalCommit !== gitCommit) {
    fail("git commit changed during retained Product runtime execution");
  }
  const finalSourceSha256 = sourceHashes();
  if (!sameValue(finalSourceSha256, initialSourceSha256)) {
    fail("Product runtime evidence source files changed during execution");
  }
  if (workingTreeStatus().trim()) {
    fail("working tree changed during retained Product runtime execution");
  }

  const completedAt = new Date().toISOString();
  const providerOutput = outputMetadata(providerProcess.output);
  const probeOutputMetadata = outputMetadata(probeOutput);
  const consumerOutput = outputMetadata(consumerProcess.output);
  writeAtomically({
    schema_version: 1,
    module: "product",
    packet: "product-catalog-separate-process-runtime-evidence",
    status: "separate_process_runtime_executed",
    generated_from: contractPath,
    runner: contract.runner,
    verifier: contract.verifier,
    git_commit: gitCommit,
    working_tree_clean_before_run: true,
    started_at: startedAt,
    completed_at: completedAt,
    toolchain: {
      node: process.version,
      cargo: cargoVersion,
      rustc: rustcVersion,
    },
    source_sha256: finalSourceSha256,
    environment_names: {
      required: contract.required_environment,
      optional_configured: contract.optional_environment.filter(
        (name) => process.env[name] !== undefined && process.env[name] !== "",
      ),
      forced: Object.keys(contract.forced_environment),
    },
    commands: contract.commands,
    provider: {
      result: "ready_and_shutdown",
      readiness_markers: contract.readiness_markers.provider,
      output_sha256: providerOutput.sha256,
      output_bytes: providerOutput.bytes,
    },
    authenticated_probe: {
      result: "passed",
      readiness_markers: contract.readiness_markers.probe,
      operation_count: 3,
      output_sha256: probeOutputMetadata.sha256,
      output_bytes: probeOutputMetadata.bytes,
    },
    consumer_server: {
      result: "remote_provider_initialized_and_shutdown",
      readiness_markers: contract.readiness_markers.consumer,
      output_sha256: consumerOutput.sha256,
      output_bytes: consumerOutput.bytes,
    },
    closed_gates: [
      "standalone_provider_postgresql_schema_preflight_runtime_evidence",
      "authenticated_product_catalog_read_rpc_evidence",
      "separate_process_consumer_remote_provider_startup_evidence",
    ],
    remaining_gates: [
      "authenticated_separate_process_commerce_business_request_evidence",
      "authenticated_separate_process_ai_business_request_evidence",
      "retained_business_end_to_end_logs_or_ci_artifacts_for_transport_promotion",
    ],
    promotion: {
      product_status: "boundary_ready",
      transport_verified_claimed: false,
      reason:
        "Provider startup, authenticated owner RPCs, and consumer remote-profile initialization do not by themselves prove Commerce or AI business requests through the separate consumer process.",
    },
  });
  console.log(`Retained Product separate-process runtime evidence written to ${contract.evidence_path}`);
} catch (error) {
  console.error(`Product separate-process runtime evidence capture failed: ${error.message}`);
  process.exitCode = 1;
} finally {
  await stopProcess(consumerProcess, defaultShutdownTimeoutMs);
  await stopProcess(providerProcess, defaultShutdownTimeoutMs);
}
