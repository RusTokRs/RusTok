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
  "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-evaluator-source.json",
);
const MAX_INPUT_BYTES = 2 * 1024 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const RESERVED_MATCHER_LABELS = new Set(["__name__", "source_commit", "operation", "outcome", "le"]);
const OPERATIONS = ["preview", "publish"];
const OUTCOMES = ["succeeded", "sanitize_failed", "runtime_failed", "other_failed"];
const THRESHOLDS = {
  preview_p95_ms: 1500,
  publish_p95_ms: 3000,
  sanitize_failure_rate_max: 0.01,
  runtime_error_rate_max: 0.01,
};
const MINIMUM_SAMPLES_PER_OPERATION = 20;
const COUNT_EPSILON = 1e-6;

function fail(message) {
  throw new Error(`Page Builder provider-health deployment evaluation failed: ${message}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function parseArguments(argv) {
  const options = {};
  const accepted = new Set([
    "--identity",
    "--backend-map",
    "--prometheus-url",
    "--window-seconds",
    "--freshness-seconds",
    "--output",
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: evaluate-page-builder-provider-health-deployment.mjs " +
          "--identity FILE --backend-map FILE --prometheus-url URL " +
          "--window-seconds N --freshness-seconds N [--output FILE]",
      );
      process.exit(0);
    }
    if (!accepted.has(argument)) fail(`unknown argument ${argument}`);
    const value = argv[index + 1];
    if (!value) fail(`${argument} requires a value`);
    options[
      argument
        .slice(2)
        .replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())
    ] = value;
    index += 1;
  }
  return options;
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
  if (!/^[0-9a-f]{40}$/u.test(value)) fail("git HEAD is not a canonical lowercase commit");
  return value;
}

function regularJsonFile(inputPath, label) {
  const absolute = path.isAbsolute(inputPath)
    ? path.resolve(inputPath)
    : path.resolve(process.cwd(), inputPath);
  if (!existsSync(absolute)) fail(`${label} is missing`);
  const metadata = lstatSync(absolute);
  if (metadata.isSymbolicLink() || !metadata.isFile()) fail(`${label} must be a regular non-symlink file`);
  const size = statSync(absolute).size;
  if (size <= 0 || size > MAX_INPUT_BYTES) fail(`${label} is outside the bounded size`);
  try {
    return JSON.parse(readFileSync(absolute, "utf8"));
  } catch (error) {
    fail(`${label} must contain JSON: ${error.message}`);
  }
}

function boundedIdentifier(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    Buffer.byteLength(value) > 128 ||
    !/^[A-Za-z0-9._:/-]+$/u.test(value)
  ) fail(`${label} must be a bounded identifier`);
  return value;
}

function exactLabelName(value, label) {
  if (typeof value !== "string" || !/^[A-Za-z_][A-Za-z0-9_]*$/u.test(value)) {
    fail(`${label} must be a Prometheus label name`);
  }
  if (RESERVED_MATCHER_LABELS.has(value)) fail(`${label} uses reserved label ${value}`);
  return value;
}

function exactLabelValue(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    Buffer.byteLength(value) > 256 ||
    !/^[A-Za-z0-9._:/@-]+$/u.test(value)
  ) fail(`${label} must be a bounded exact-match label value`);
  return value;
}

function requireIdentity(document, head) {
  if (document?.format !== "page_builder_provider_health_deployment_identity_v1") {
    fail("identity packet format is not page_builder_provider_health_deployment_identity_v1");
  }
  if (document.status !== "deployment_identity_verified_health_evaluation_pending") {
    fail("identity packet is not admitted for health evaluation");
  }
  const deployment = document.deployment ?? {};
  if (deployment.source_commit !== head) fail("identity source commit does not match git HEAD");
  if (deployment.inventory_complete !== true) fail("identity inventory is not complete");
  if (
    !Number.isInteger(deployment.expected_target_count) ||
    deployment.expected_target_count < 1 ||
    deployment.expected_target_count > 64 ||
    deployment.expected_target_count !== deployment.verified_target_count
  ) fail("identity expected/verified target counts are invalid");
  if (
    typeof deployment.deployment_image_digest !== "string" ||
    !/^[^\s@]+@sha256:[0-9a-f]{64}$/u.test(deployment.deployment_image_digest)
  ) fail("identity deployment image digest is invalid");
  if (!Array.isArray(document.expected_targets) || document.expected_targets.length !== deployment.expected_target_count) {
    fail("identity expected target list does not match admitted target count");
  }
  const targetIds = new Set();
  for (const [index, target] of document.expected_targets.entries()) {
    const targetId = boundedIdentifier(target?.target_id, `identity.expected_targets[${index}].target_id`);
    if (targetIds.has(targetId)) fail(`identity contains duplicate target ${targetId}`);
    if (target?.source_commit_verified_equal_checkout !== true || target?.reported_source_commit !== head) {
      fail(`identity target ${targetId} is not bound to checkout source commit`);
    }
    targetIds.add(targetId);
  }
  const capturedAtMs = Date.parse(document.captured_at);
  if (!Number.isFinite(capturedAtMs)) fail("identity captured_at is invalid");
  return { deployment, targetIds, capturedAtSeconds: capturedAtMs / 1000 };
}

function requireBackendMap(document, identity) {
  if (document?.schema_version !== 1) fail("backend map schema_version must be 1");
  if (document.deployment_id !== identity.deployment.deployment_id) {
    fail("backend map deployment_id does not match identity packet");
  }
  if (document.inventory_complete !== true) fail("backend map inventory_complete must be true");
  const targetLabel = exactLabelName(document.target_label, "backend map target_label");
  const commonMatchers = document.common_matchers ?? {};
  if (commonMatchers === null || typeof commonMatchers !== "object" || Array.isArray(commonMatchers)) {
    fail("backend map common_matchers must be an object");
  }
  const commonEntries = Object.entries(commonMatchers);
  if (commonEntries.length > 8) fail("backend map common_matchers exceeds 8 entries");
  const normalizedCommon = {};
  for (const [name, value] of commonEntries) {
    const matcherName = exactLabelName(name, `common matcher ${name}`);
    if (matcherName === targetLabel) fail("target label must not be duplicated in common_matchers");
    normalizedCommon[matcherName] = exactLabelValue(value, `common matcher ${name}`);
  }
  if (!Array.isArray(document.targets) || document.targets.length !== identity.targetIds.size) {
    fail("backend map targets must exactly cover the identity target count");
  }
  const mappedIds = new Set();
  const targetValues = new Set();
  const targets = document.targets.map((target, index) => {
    const targetId = boundedIdentifier(target?.target_id, `backend targets[${index}].target_id`);
    if (!identity.targetIds.has(targetId)) fail(`backend map contains unknown target ${targetId}`);
    if (mappedIds.has(targetId)) fail(`backend map duplicates target ${targetId}`);
    const targetValue = exactLabelValue(target?.target_label_value, `backend targets[${index}].target_label_value`);
    if (targetValues.has(targetValue)) fail(`backend map duplicates target label value ${targetValue}`);
    mappedIds.add(targetId);
    targetValues.add(targetValue);
    return { target_id: targetId, target_label_value: targetValue };
  });
  if (mappedIds.size !== identity.targetIds.size) fail("backend map target set is incomplete");
  return { targetLabel, commonMatchers: normalizedCommon, targets };
}

function requirePrometheusUrl(value) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail("--prometheus-url must be an absolute HTTP(S) URL");
  }
  if (!["http:", "https:"].includes(parsed.protocol) || parsed.username || parsed.password || parsed.search || parsed.hash) {
    fail("--prometheus-url must not contain credentials, query, or fragment");
  }
  if (!parsed.pathname.endsWith("/")) parsed.pathname += "/";
  return parsed;
}

function boundedInteger(value, label, minimum, maximum) {
  if (!/^[0-9]+$/u.test(value ?? "")) fail(`${label} must be an integer`);
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < minimum || parsed > maximum) {
    fail(`${label} must be between ${minimum} and ${maximum}`);
  }
  return parsed;
}

function credentialHeaders(contract) {
  const headers = {
    accept: "application/json",
    "content-type": "application/x-www-form-urlencoded",
    "cache-control": "no-cache",
    pragma: "no-cache",
  };
  const environmentNames = [];
  const env = contract.backend_query.credential_environment;
  const common = process.env[env.common_headers_json];
  if (common) {
    if (common.length > 16_384 || /[\u0000]/u.test(common)) fail(`${env.common_headers_json} is outside the bounded input`);
    let parsed;
    try {
      parsed = JSON.parse(common);
    } catch (error) {
      fail(`${env.common_headers_json} must contain a JSON object: ${error.message}`);
    }
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) fail(`${env.common_headers_json} must contain an object`);
    for (const [name, value] of Object.entries(parsed)) {
      const normalized = name.toLowerCase();
      if (!/^[a-z0-9!#$%&'*+.^_`|~-]+$/u.test(normalized)) fail(`invalid common header ${name}`);
      if (["authorization", "cookie", "host", "content-length"].includes(normalized)) fail(`common headers must not contain ${normalized}`);
      if (typeof value !== "string" || value.length > 4096 || /[\r\n\u0000]/u.test(value)) fail(`common header ${name} is outside the bounded input`);
      headers[normalized] = value;
    }
    environmentNames.push(env.common_headers_json);
  }
  for (const [field, header, limit] of [
    ["authorization", "authorization", 8192],
    ["cookie", "cookie", 16_384],
  ]) {
    const envName = env[field];
    const value = process.env[envName];
    if (!value) continue;
    if (value.length > limit || /[\r\n\u0000]/u.test(value)) fail(`${envName} is outside the bounded input`);
    headers[header] = value;
    environmentNames.push(envName);
  }
  return { headers, environment_names: [...new Set(environmentNames)].sort() };
}

function quoted(value) {
  return value.replaceAll("\\", "\\\\").replaceAll('"', '\\"');
}

function selector(commonMatchers, targetLabel, targetValue, extraEquals = {}) {
  const all = { ...commonMatchers, [targetLabel]: targetValue, ...extraEquals };
  const entries = Object.entries(all).sort(([left], [right]) => left.localeCompare(right));
  return `{${entries.map(([name, value]) => `${name}="${quoted(value)}"`).join(",")}}`;
}

function selectorExcludingSource(baseSelector, sourceCommit) {
  return `${baseSelector.slice(0, -1)},source_commit!="${sourceCommit}"}`;
}

async function queryPrometheus(baseUrl, query, credentials, contract) {
  const endpoint = new URL("api/v1/query", baseUrl);
  let response;
  try {
    response = await fetch(endpoint, {
      method: "POST",
      headers: credentials.headers,
      body: new URLSearchParams({ query }).toString(),
      redirect: "manual",
      signal: AbortSignal.timeout(contract.backend_query.request_timeout_ms),
    });
  } catch (error) {
    fail(`Prometheus query request failed: ${error.message}`);
  }
  if (response.status !== 200) fail(`Prometheus query expected 200, got ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  if (bytes.byteLength <= 0 || bytes.byteLength > contract.backend_query.maximum_response_bytes) {
    fail("Prometheus response is outside the bounded body size");
  }
  let payload;
  try {
    payload = JSON.parse(bytes.toString("utf8"));
  } catch (error) {
    fail(`Prometheus response is not JSON: ${error.message}`);
  }
  if (payload?.status !== "success" || payload?.data?.resultType !== "vector" || !Array.isArray(payload.data.result)) {
    fail("Prometheus query did not return a successful vector result");
  }
  return payload.data.result;
}

function numericSample(result, label) {
  if (!Array.isArray(result?.value) || result.value.length < 2) fail(`${label} has no instant value`);
  const value = Number(result.value[1]);
  if (!Number.isFinite(value) || value < 0) fail(`${label} is non-finite or negative`);
  return value;
}

function fingerprint(metric) {
  return sha256(JSON.stringify(Object.entries(metric ?? {}).sort(([a], [b]) => a.localeCompare(b))));
}

function parseFreshness(results, targetId) {
  const values = new Map();
  for (const result of results) {
    const operation = result?.metric?.operation;
    if (!OPERATIONS.includes(operation)) fail(`${targetId} freshness returned unknown operation ${operation}`);
    if (values.has(operation)) fail(`${targetId} freshness returned duplicate ${operation} series`);
    values.set(operation, numericSample(result, `${targetId} ${operation} freshness`));
  }
  for (const operation of OPERATIONS) {
    if (!values.has(operation)) fail(`${targetId} is missing ${operation} freshness`);
  }
  return values;
}

function zeroCompletions() {
  return Object.fromEntries(
    OPERATIONS.map((operation) => [operation, Object.fromEntries(OUTCOMES.map((outcome) => [outcome, 0]))]),
  );
}

function parseCompletions(results, targetId) {
  const totals = zeroCompletions();
  for (const result of results) {
    const operation = result?.metric?.operation;
    const outcome = result?.metric?.outcome;
    if (!OPERATIONS.includes(operation)) fail(`${targetId} completion returned unknown operation ${operation}`);
    if (!OUTCOMES.includes(outcome)) fail(`${targetId} completion returned unknown outcome ${outcome}`);
    totals[operation][outcome] += numericSample(result, `${targetId} ${operation}/${outcome} completion`);
  }
  return totals;
}

function parseBuckets(results, targetId) {
  const buckets = { preview: new Map(), publish: new Map() };
  for (const result of results) {
    const operation = result?.metric?.operation;
    const le = result?.metric?.le;
    if (!OPERATIONS.includes(operation)) fail(`${targetId} histogram returned unknown operation ${operation}`);
    if (typeof le !== "string" || (le !== "+Inf" && !Number.isFinite(Number(le)))) {
      fail(`${targetId} histogram returned invalid le=${le}`);
    }
    const value = numericSample(result, `${targetId} ${operation} histogram bucket ${le}`);
    buckets[operation].set(le, (buckets[operation].get(le) ?? 0) + value);
  }
  return buckets;
}

function operationTotal(completions, operation) {
  return OUTCOMES.reduce((total, outcome) => total + completions[operation][outcome], 0);
}

function addCompletions(destination, source) {
  for (const operation of OPERATIONS) {
    for (const outcome of OUTCOMES) destination[operation][outcome] += source[operation][outcome];
  }
}

function addBuckets(destination, source) {
  for (const operation of OPERATIONS) {
    for (const [le, value] of source[operation]) {
      destination[operation].set(le, (destination[operation].get(le) ?? 0) + value);
    }
  }
}

function requireHistogramPopulationConsistency(completions, buckets) {
  for (const operation of OPERATIONS) {
    if (!buckets[operation].has("+Inf")) fail(`${operation} histogram is missing +Inf bucket`);
    const completionCount = operationTotal(completions, operation);
    const histogramCount = buckets[operation].get("+Inf");
    const tolerance = Math.max(COUNT_EPSILON, completionCount * COUNT_EPSILON);
    if (Math.abs(completionCount - histogramCount) > tolerance) {
      fail(`${operation} histogram +Inf population does not match terminal completion population`);
    }
  }
}

function histogramQuantile95(bucketMap, operation) {
  if (!bucketMap.has("+Inf")) fail(`${operation} histogram is missing +Inf bucket`);
  const finite = [...bucketMap.entries()]
    .filter(([le]) => le !== "+Inf")
    .map(([le, value]) => [Number(le), value])
    .sort(([left], [right]) => left - right);
  if (finite.length === 0) fail(`${operation} histogram has no finite buckets`);
  const total = bucketMap.get("+Inf");
  if (total < MINIMUM_SAMPLES_PER_OPERATION) {
    fail(`${operation} histogram has fewer than ${MINIMUM_SAMPLES_PER_OPERATION} samples`);
  }
  let previousCumulative = 0;
  for (const [, cumulative] of finite) {
    if (cumulative + COUNT_EPSILON < previousCumulative) fail(`${operation} histogram buckets are not cumulative`);
    previousCumulative = cumulative;
  }
  if (total + COUNT_EPSILON < previousCumulative) fail(`${operation} +Inf bucket is below finite cumulative count`);

  const rank = total * 0.95;
  let lowerBound = 0;
  let lowerCumulative = 0;
  for (const [upperBound, cumulative] of finite) {
    if (cumulative >= rank) {
      const bucketCount = cumulative - lowerCumulative;
      if (bucketCount <= COUNT_EPSILON) return Math.ceil(upperBound * 1000);
      const fraction = Math.max(0, Math.min(1, (rank - lowerCumulative) / bucketCount));
      return Math.ceil((lowerBound + (upperBound - lowerBound) * fraction) * 1000);
    }
    lowerBound = upperBound;
    lowerCumulative = cumulative;
  }
  return Math.ceil(finite[finite.length - 1][0] * 1000);
}

function status(pass) {
  return pass ? "pass" : "fail";
}

function evaluateHealth(observed) {
  const degradationReasons = [];
  if (
    observed.preview_p95_ms > THRESHOLDS.preview_p95_ms ||
    observed.runtime_error_rate > THRESHOLDS.runtime_error_rate_max
  ) degradationReasons.push("provider_unhealthy");
  if (observed.sanitize_failure_rate > THRESHOLDS.sanitize_failure_rate_max) {
    degradationReasons.push("sanitize_backpressure");
  }
  if (observed.publish_p95_ms > THRESHOLDS.publish_p95_ms) {
    degradationReasons.push("publish_backlog");
  }
  const state = degradationReasons.length === 0
    ? "ready"
    : observed.runtime_error_rate > THRESHOLDS.runtime_error_rate_max * 2.0
      ? "unavailable"
      : "degraded";
  const sloEvaluation = {
    preview_p95_ms: status(observed.preview_p95_ms <= THRESHOLDS.preview_p95_ms),
    publish_p95_ms: status(observed.publish_p95_ms <= THRESHOLDS.publish_p95_ms),
    sanitize_failure_rate: status(observed.sanitize_failure_rate <= THRESHOLDS.sanitize_failure_rate_max),
    runtime_error_rate: status(observed.runtime_error_rate <= THRESHOLDS.runtime_error_rate_max),
  };
  sloEvaluation.overall = status(Object.values(sloEvaluation).every((value) => value === "pass"));
  return {
    snapshot: {
      state,
      degradation_reasons: degradationReasons,
      thresholds: THRESHOLDS,
      observed,
    },
    slo_evaluation: sloEvaluation,
  };
}

function regularSourceHash(relativePath) {
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) fail(`source file ${relativePath} escapes repository root`);
  if (!existsSync(absolute)) fail(`source file ${relativePath} is missing`);
  const metadata = lstatSync(absolute);
  if (metadata.isSymbolicLink() || !metadata.isFile()) fail(`source file ${relativePath} must be a regular non-symlink file`);
  const size = statSync(absolute).size;
  if (size <= 0 || size > MAX_SOURCE_BYTES) fail(`source file ${relativePath} is outside the bounded source size`);
  return sha256(readFileSync(absolute));
}

function sourceHashes(contract) {
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => [relativePath, regularSourceHash(relativePath)]),
  );
}

function outputPath(contract, requested) {
  const candidate = requested ?? contract.output.default_path;
  if (typeof candidate !== "string" || candidate.length === 0 || candidate.length > 16_384) {
    fail("evaluation output path is invalid");
  }
  const absolute = path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) fail("evaluation output must remain inside repository target/");
  return absolute;
}

function writeAtomic(location, document) {
  mkdirSync(path.dirname(location), { recursive: true });
  const temporary = `${location}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, location);
}

async function evaluateTarget(target, context) {
  const baseSelector = selector(
    context.backendMap.commonMatchers,
    context.backendMap.targetLabel,
    target.target_label_value,
  );
  const sourceSelector = selector(
    context.backendMap.commonMatchers,
    context.backendMap.targetLabel,
    target.target_label_value,
    { source_commit: context.identity.deployment.source_commit },
  );

  const currentBuild = await queryPrometheus(
    context.prometheusUrl,
    `rustok_page_builder_provider_build_info${sourceSelector}`,
    context.credentials,
    context.contract,
  );
  if (currentBuild.length !== 1 || numericSample(currentBuild[0], `${target.target_id} current build-info`) !== 1) {
    fail(`${target.target_id} must expose exactly one current admitted build-info series equal to 1`);
  }

  const admittedWindow = await queryPrometheus(
    context.prometheusUrl,
    `count_over_time(rustok_page_builder_provider_build_info${sourceSelector}[${context.windowSeconds}s])`,
    context.credentials,
    context.contract,
  );
  if (admittedWindow.length !== 1 || numericSample(admittedWindow[0], `${target.target_id} admitted source window`) <= 0) {
    fail(`${target.target_id} has no admitted source build-info samples in the query window`);
  }

  const unexpectedWindow = await queryPrometheus(
    context.prometheusUrl,
    `count_over_time(rustok_page_builder_provider_build_info${selectorExcludingSource(
      baseSelector,
      context.identity.deployment.source_commit,
    )}[${context.windowSeconds}s])`,
    context.credentials,
    context.contract,
  );
  if (unexpectedWindow.some((result) => numericSample(result, `${target.target_id} unexpected source window`) > 0)) {
    fail(`${target.target_id} observed an unexpected source commit inside the query window`);
  }

  const freshness = parseFreshness(
    await queryPrometheus(
      context.prometheusUrl,
      `rustok_page_builder_provider_last_observation_unix_seconds${baseSelector}`,
      context.credentials,
      context.contract,
    ),
    target.target_id,
  );
  const freshnessAges = {};
  for (const operation of OPERATIONS) {
    const timestamp = freshness.get(operation);
    if (timestamp > context.backendNow + 5) fail(`${target.target_id} ${operation} freshness is in the future`);
    const age = context.backendNow - timestamp;
    if (age > context.freshnessSeconds) fail(`${target.target_id} ${operation} freshness is stale (${age}s)`);
    freshnessAges[operation] = age;
  }

  const completions = parseCompletions(
    await queryPrometheus(
      context.prometheusUrl,
      `increase(rustok_page_builder_provider_operation_completed_total${baseSelector}[${context.windowSeconds}s])`,
      context.credentials,
      context.contract,
    ),
    target.target_id,
  );
  const buckets = parseBuckets(
    await queryPrometheus(
      context.prometheusUrl,
      `increase(rustok_page_builder_provider_operation_duration_seconds_bucket${baseSelector}[${context.windowSeconds}s])`,
      context.credentials,
      context.contract,
    ),
    target.target_id,
  );
  requireHistogramPopulationConsistency(completions, buckets);

  return {
    target_id: target.target_id,
    selector_sha256: sha256(baseSelector),
    raw_selector_persisted: false,
    backend_series_fingerprint_sha256: fingerprint(currentBuild[0].metric),
    current_source_commit_verified: true,
    unexpected_source_in_window: false,
    preview_freshness_age_seconds: freshnessAges.preview,
    publish_freshness_age_seconds: freshnessAges.publish,
    completions,
    buckets,
  };
}

async function mapWithConcurrency(items, limit, mapper) {
  const results = new Array(items.length);
  let next = 0;
  async function worker() {
    while (true) {
      const index = next;
      next += 1;
      if (index >= items.length) return;
      results[index] = await mapper(items[index]);
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, () => worker()));
  return results;
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  for (const required of ["identity", "backendMap", "prometheusUrl", "windowSeconds", "freshnessSeconds"]) {
    if (!options[required]) {
      fail(`--${required.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)} is required`);
    }
  }

  const contract = JSON.parse(readFileSync(contractPath, "utf8"));
  if (contract.status !== "source_ready_execution_pending") {
    fail("evaluator source contract must remain execution-pending before runtime evaluation");
  }
  const head = currentCommit();
  const identity = requireIdentity(regularJsonFile(options.identity, "identity packet"), head);
  const backendMap = requireBackendMap(regularJsonFile(options.backendMap, "backend target map"), identity);
  const prometheusUrl = requirePrometheusUrl(options.prometheusUrl);
  const windowSeconds = boundedInteger(options.windowSeconds, "--window-seconds", 300, 86400);
  const freshnessSeconds = boundedInteger(options.freshnessSeconds, "--freshness-seconds", 60, windowSeconds);
  const credentials = credentialHeaders(contract);
  const output = outputPath(contract, options.output);
  rmSync(output, { force: true });

  const timeResult = await queryPrometheus(prometheusUrl, "time()", credentials, contract);
  if (timeResult.length !== 1) fail("Prometheus time() must return exactly one sample");
  const backendNow = numericSample(timeResult[0], "Prometheus time()");
  const identityAge = backendNow - identity.capturedAtSeconds;
  if (identityAge < windowSeconds) fail("identity capture must predate the entire query window");
  if (identityAge > contract.backend_query.identity_capture_maximum_age_seconds) {
    fail("identity capture is older than the admitted maximum age");
  }

  const targetResults = await mapWithConcurrency(
    backendMap.targets,
    contract.backend_query.maximum_parallel_queries,
    (target) => evaluateTarget(target, {
      contract,
      identity,
      backendMap,
      prometheusUrl,
      credentials,
      windowSeconds,
      freshnessSeconds,
      backendNow,
    }),
  );
  if (targetResults.length !== identity.targetIds.size) fail("partial target evaluation is forbidden");

  const fingerprints = new Set();
  for (const result of targetResults) {
    if (fingerprints.has(result.backend_series_fingerprint_sha256)) {
      fail("multiple expected target ids resolve to the same current backend series");
    }
    fingerprints.add(result.backend_series_fingerprint_sha256);
  }

  const completions = zeroCompletions();
  const buckets = { preview: new Map(), publish: new Map() };
  for (const target of targetResults) {
    addCompletions(completions, target.completions);
    addBuckets(buckets, target.buckets);
  }
  requireHistogramPopulationConsistency(completions, buckets);

  const previewSamples = operationTotal(completions, "preview");
  const publishSamples = operationTotal(completions, "publish");
  if (previewSamples < MINIMUM_SAMPLES_PER_OPERATION || publishSamples < MINIMUM_SAMPLES_PER_OPERATION) {
    fail(`deployment sample floor requires at least ${MINIMUM_SAMPLES_PER_OPERATION} preview and publish completions`);
  }

  const sanitizeFailures = completions.publish.sanitize_failed;
  const runtimeFailures = completions.preview.runtime_failed + completions.publish.runtime_failed;
  const observed = {
    preview_p95_ms: histogramQuantile95(buckets.preview, "preview"),
    publish_p95_ms: histogramQuantile95(buckets.publish, "publish"),
    sanitize_failure_rate: sanitizeFailures / publishSamples,
    runtime_error_rate: runtimeFailures / (previewSamples + publishSamples),
  };
  for (const [name, value] of Object.entries(observed)) {
    if (!Number.isFinite(value) || value < 0) fail(`computed observation ${name} is non-finite or negative`);
  }
  const evaluated = evaluateHealth(observed);

  writeAtomic(output, {
    format: contract.output.format,
    status: contract.output.status,
    evaluated_at: new Date(backendNow * 1000).toISOString(),
    deployment: {
      deployment_id: identity.deployment.deployment_id,
      deployment_image_digest: identity.deployment.deployment_image_digest,
      source_commit: identity.deployment.source_commit,
      identity_captured_at: new Date(identity.capturedAtSeconds * 1000).toISOString(),
      identity_age_seconds: identityAge,
      expected_target_count: identity.targetIds.size,
      verified_backend_target_count: targetResults.length,
      query_window_seconds: windowSeconds,
      freshness_seconds: freshnessSeconds,
    },
    backend: {
      prometheus_url_bytes: Buffer.byteLength(prometheusUrl.href),
      prometheus_url_sha256: sha256(prometheusUrl.href),
      raw_prometheus_url_persisted: false,
      target_label_name: backendMap.targetLabel,
      common_matcher_names: Object.keys(backendMap.commonMatchers).sort(),
      raw_matcher_values_persisted: false,
      raw_promql_persisted: false,
      raw_backend_responses_persisted: false,
      target_mapping_complete: true,
    },
    targets: targetResults.map(({ completions: _completions, buckets: _buckets, ...retained }) => retained),
    samples: {
      preview: previewSamples,
      publish: publishSamples,
      preview_histogram: buckets.preview.get("+Inf"),
      publish_histogram: buckets.publish.get("+Inf"),
      minimum_per_operation: MINIMUM_SAMPLES_PER_OPERATION,
    },
    ...evaluated,
    source_files: sourceHashes(contract),
    credentials: {
      environment_names: credentials.environment_names,
      values_persisted: false,
    },
    pages_provider_health_observed: false,
    pages_reference_consumer_gate_accepted: false,
    forum_wave_accepted: false,
    ffa_promoted: false,
    fba_promoted: false,
  });
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
