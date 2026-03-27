#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/commerce_rollout_report.sh [options]

Options:
  --artifacts-dir <dir>            Output folder for logs/report (default: artifacts/commerce-rollout-report)
  --env <name>                     Environment label in report (default: local)
  --base-url <url>                 Base server URL for live evidence (default: http://localhost:5150)
  --metrics-url <url>              Explicit /metrics URL (overrides --base-url)
  --runtime-url <url>              Explicit /health/runtime URL (overrides --base-url)
  --metrics-snapshot <file>        Use existing /metrics snapshot instead of fetching
  --runtime-snapshot <file>        Use existing /health/runtime snapshot instead of fetching
  --parity-report <file>           Path to existing parity report evidence
  --skip-local-tests               Skip local regression slice and mark it pending
  --max-legacy-share-percent <n>   Usage evidence threshold (default: 10)
  --out <file>                     Output markdown report path
  --summary-out <file>             Output JSON summary path
  --help                           Show this message

Environment:
  RUSTOK_CARGO_BIN                 Override cargo executable path (default: cargo)
  RUSTOK_NODE_BIN                  Override node executable path (default: node)

Examples:
  scripts/commerce_rollout_report.sh --base-url http://localhost:5150
  scripts/commerce_rollout_report.sh \
    --metrics-snapshot artifacts/staging/metrics.prom \
    --runtime-snapshot artifacts/staging/runtime.json \
    --parity-report artifacts/staging/commerce-parity.md
USAGE
}

ARTIFACTS_DIR="artifacts/commerce-rollout-report"
ENV_NAME="local"
BASE_URL="http://localhost:5150"
METRICS_URL=""
RUNTIME_URL=""
METRICS_SNAPSHOT=""
RUNTIME_SNAPSHOT=""
PARITY_REPORT=""
SKIP_LOCAL_TESTS="false"
MAX_LEGACY_SHARE_PERCENT="10"
OUT_FILE=""
SUMMARY_OUT=""
CARGO_BIN="${RUSTOK_CARGO_BIN:-cargo}"
NODE_BIN="${RUSTOK_NODE_BIN:-node}"
LOCAL_TEST_FAILURE="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --artifacts-dir)
      ARTIFACTS_DIR="$2"; shift 2 ;;
    --env)
      ENV_NAME="$2"; shift 2 ;;
    --base-url)
      BASE_URL="$2"; shift 2 ;;
    --metrics-url)
      METRICS_URL="$2"; shift 2 ;;
    --runtime-url)
      RUNTIME_URL="$2"; shift 2 ;;
    --metrics-snapshot)
      METRICS_SNAPSHOT="$2"; shift 2 ;;
    --runtime-snapshot)
      RUNTIME_SNAPSHOT="$2"; shift 2 ;;
    --parity-report)
      PARITY_REPORT="$2"; shift 2 ;;
    --skip-local-tests)
      SKIP_LOCAL_TESTS="true"; shift ;;
    --max-legacy-share-percent)
      MAX_LEGACY_SHARE_PERCENT="$2"; shift 2 ;;
    --out)
      OUT_FILE="$2"; shift 2 ;;
    --summary-out)
      SUMMARY_OUT="$2"; shift 2 ;;
    --help)
      usage; exit 0 ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1 ;;
  esac
done

if ! command -v "$NODE_BIN" >/dev/null 2>&1; then
  echo "Node.js executable not found: $NODE_BIN" >&2
  exit 1
fi

mkdir -p "$ARTIFACTS_DIR"
TS="$(date -u +%Y%m%dT%H%M%SZ)"
if [[ -z "$OUT_FILE" ]]; then
  OUT_FILE="$ARTIFACTS_DIR/commerce_rollout_report_${TS}.md"
fi
if [[ -z "$SUMMARY_OUT" ]]; then
  SUMMARY_OUT="$ARTIFACTS_DIR/commerce_rollout_summary_${TS}.json"
fi

if [[ -z "$METRICS_URL" ]]; then
  METRICS_URL="${BASE_URL%/}/metrics"
fi
if [[ -z "$RUNTIME_URL" ]]; then
  RUNTIME_URL="${BASE_URL%/}/health/runtime"
fi

ROLL_OUT_LOG="$ARTIFACTS_DIR/commerce_rollout_middleware_${TS}.log"
OPENAPI_LOG="$ARTIFACTS_DIR/commerce_openapi_contract_${TS}.log"
LEGACY_OPENAPI_LOG="$ARTIFACTS_DIR/commerce_legacy_openapi_contract_${TS}.log"
SCHEMA_SMOKE_LOG="$ARTIFACTS_DIR/ecommerce_schema_smoke_${TS}.log"
STARTUP_LOG="$ARTIFACTS_DIR/startup_router_smoke_${TS}.log"

FETCHED_METRICS_FILE="$ARTIFACTS_DIR/commerce_metrics_${TS}.prom"
FETCHED_RUNTIME_FILE="$ARTIFACTS_DIR/commerce_runtime_${TS}.json"

integration_status="pending"
integration_note="Skipped by flag --skip-local-tests"
declare -a failed_suites=()

run_local_suite() {
  local log_file="$1"
  shift
  if ! "$CARGO_BIN" test "$@" >"$log_file" 2>&1; then
    failed_suites+=("$(basename "$log_file" | sed -E 's/_[0-9]{8}T[0-9]{6}Z\.log$//')")
    LOCAL_TEST_FAILURE="true"
  fi
}

if [[ "$SKIP_LOCAL_TESTS" != "true" ]]; then
  integration_status="done"
  integration_note="cargo test ecommerce rollout middleware + store/legacy OpenAPI + migration smoke + router startup"

  run_local_suite "$ROLL_OUT_LOG" -p rustok-server --test commerce_rollout_middleware_test
  run_local_suite "$OPENAPI_LOG" -p rustok-server --test commerce_openapi_contract
  run_local_suite "$LEGACY_OPENAPI_LOG" -p rustok-server --test commerce_legacy_openapi_contract
  run_local_suite "$SCHEMA_SMOKE_LOG" -p migration --test ecommerce_schema_smoke
  run_local_suite "$STARTUP_LOG" -p rustok-server startup_smoke_builds_router_and_runtime_shared_state --lib

  if [[ ${#failed_suites[@]} -gt 0 ]]; then
    integration_status="failed"
    integration_note="Failed suites: ${failed_suites[*]} (see logs)"
  fi
else
  echo "Skipped (--skip-local-tests)." >"$ROLL_OUT_LOG"
  echo "Skipped (--skip-local-tests)." >"$OPENAPI_LOG"
  echo "Skipped (--skip-local-tests)." >"$LEGACY_OPENAPI_LOG"
  echo "Skipped (--skip-local-tests)." >"$SCHEMA_SMOKE_LOG"
  echo "Skipped (--skip-local-tests)." >"$STARTUP_LOG"
fi

metrics_source="provided"
if [[ -z "$METRICS_SNAPSHOT" ]]; then
  metrics_source="$METRICS_URL"
  if curl -fsSL "$METRICS_URL" -o "$FETCHED_METRICS_FILE"; then
    METRICS_SNAPSHOT="$FETCHED_METRICS_FILE"
  else
    METRICS_SNAPSHOT="$FETCHED_METRICS_FILE.fetch_failed"
  fi
fi

runtime_source="provided"
if [[ -z "$RUNTIME_SNAPSHOT" ]]; then
  runtime_source="$RUNTIME_URL"
  if curl -fsSL "$RUNTIME_URL" -o "$FETCHED_RUNTIME_FILE"; then
    RUNTIME_SNAPSHOT="$FETCHED_RUNTIME_FILE"
  else
    RUNTIME_SNAPSHOT="$FETCHED_RUNTIME_FILE.fetch_failed"
  fi
fi

NODE_OUTPUT=$("$NODE_BIN" - \
  "$METRICS_SNAPSHOT" \
  "$RUNTIME_SNAPSHOT" \
  "$PARITY_REPORT" \
  "$ENV_NAME" \
  "$TS" \
  "$MAX_LEGACY_SHARE_PERCENT" \
  "$integration_status" \
  "$integration_note" \
  "$metrics_source" \
  "$runtime_source" \
  "$ROLL_OUT_LOG" \
  "$OPENAPI_LOG" \
  "$LEGACY_OPENAPI_LOG" \
  "$SCHEMA_SMOKE_LOG" \
  "$STARTUP_LOG" <<'JS'
const fs = require('fs');

const [
  metricsPathRaw,
  runtimePathRaw,
  parityReportRaw,
  envName,
  timestampUtc,
  maxLegacySharePercentRaw,
  integrationStatus,
  integrationNote,
  metricsSource,
  runtimeSource,
  rolloutLog,
  openapiLog,
  legacyOpenapiLog,
  schemaSmokeLog,
  startupLog,
] = process.argv.slice(2);

const maxLegacySharePercent = Number(maxLegacySharePercentRaw);

function titleCase(value) {
  if (!value) return 'Unknown';
  return value.charAt(0).toUpperCase() + value.slice(1);
}

function done(note) {
  return { status: 'done', note };
}

function pending(note) {
  return { status: 'pending', note };
}

function failed(note) {
  return { status: 'failed', note };
}

function statusRank(status) {
  return { done: 0, pending: 1, failed: 2 }[status] ?? 2;
}

function parseMetricLabels(labelsRaw) {
  const labels = {};
  const regex = /(\w+)="([^"]*)"/g;
  let match;
  while ((match = regex.exec(labelsRaw)) !== null) {
    labels[match[1]] = match[2];
  }
  return labels;
}

function readMetrics(pathRaw) {
  if (!pathRaw || !fs.existsSync(pathRaw)) {
    return [null, failed(`Metrics snapshot is missing: ${pathRaw}`)];
  }

  const rollout = {};
  const entrypoints = {};
  const content = fs.readFileSync(pathRaw, 'utf8');
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    const match = line.match(/^([a-zA-Z_:][a-zA-Z0-9_:]*)\{([^}]*)\}\s+([0-9]+(?:\.[0-9]+)?)$/);
    if (!match) continue;

    const [, metricName, labelsRaw, valueRaw] = match;
    const labels = parseMetricLabels(labelsRaw);
    const value = Number(valueRaw);

    if (metricName.startsWith('rustok_runtime_guardrail_commerce_surface_')) {
      const surface = labels.surface;
      if (!surface) continue;
      if (!rollout[surface]) rollout[surface] = {};
      if (metricName.endsWith('_enabled')) {
        rollout[surface].enabled = Math.trunc(value);
      } else if (metricName.endsWith('_canary_percent')) {
        rollout[surface].canary_percent = Math.trunc(value);
      } else if (metricName.endsWith('_restricted')) {
        rollout[surface].restricted = Math.trunc(value);
      }
    }

    if (
      metricName === 'rustok_module_entrypoint_calls_total' &&
      labels.module === 'commerce'
    ) {
      const entryPoint = labels.entry_point;
      const pathLabel = labels.path;
      if (!entryPoint || !pathLabel) continue;
      const key = `${entryPoint}:${pathLabel}`;
      entrypoints[key] = (entrypoints[key] ?? 0) + value;
    }
  }

  const requiredSurfaces = ['legacy', 'store', 'admin'];
  const missingSurfaces = requiredSurfaces.filter((surface) => !rollout[surface]);
  if (missingSurfaces.length > 0) {
    return [null, failed(`Metrics snapshot is missing rollout series for surfaces: ${missingSurfaces.join(', ')}`)];
  }

  for (const surface of requiredSurfaces) {
    const missingFields = ['enabled', 'canary_percent', 'restricted'].filter(
      (field) => rollout[surface][field] === undefined
    );
    if (missingFields.length > 0) {
      return [
        null,
        failed(`Metrics snapshot is missing rollout fields for surface \`${surface}\`: ${missingFields.join(', ')}`),
      ];
    }
  }

  return [{ rollout, entrypoints }, done(`Evidence: ${pathRaw}`)];
}

function readRuntime(pathRaw) {
  if (!pathRaw || !fs.existsSync(pathRaw)) {
    return [null, failed(`Runtime snapshot is missing: ${pathRaw}`)];
  }

  let payload;
  try {
    payload = JSON.parse(fs.readFileSync(pathRaw, 'utf8'));
  } catch (error) {
    return [null, failed(`Runtime snapshot is not valid JSON: ${error.message}`)];
  }

  const surfacesRaw = payload?.ecommerce_rollout?.surfaces;
  if (!Array.isArray(surfacesRaw)) {
    return [null, failed('Runtime snapshot is missing ecommerce_rollout.surfaces')];
  }

  const surfaces = {};
  for (const item of surfacesRaw) {
    if (!item || typeof item !== 'object' || !item.surface) continue;
    surfaces[item.surface] = {
      enabled: Boolean(item.enabled),
      canary_percent: Number(item.canary_percent ?? 0),
      restricted: Boolean(item.restricted),
    };
  }

  const requiredSurfaces = ['legacy', 'store', 'admin'];
  const missingSurfaces = requiredSurfaces.filter((surface) => !surfaces[surface]);
  if (missingSurfaces.length > 0) {
    return [null, failed(`Runtime snapshot is missing rollout surfaces: ${missingSurfaces.join(', ')}`)];
  }

  const reasons = Array.isArray(payload.reasons)
    ? payload.reasons.filter((reason) => typeof reason === 'string')
    : [];

  return [
    {
      status: payload.status ?? 'unknown',
      observed_status: payload.observed_status ?? 'unknown',
      rollout: payload.rollout ?? 'unknown',
      surfaces,
      reasons,
    },
    done(`Evidence: ${pathRaw}`),
  ];
}

const [metricsData, metricsResult] = readMetrics(metricsPathRaw);
const [runtimeData, runtimeResult] = readRuntime(runtimePathRaw);

let parityResult;
if (!parityReportRaw) {
  parityResult = pending('Attach parity report via --parity-report');
} else if (fs.existsSync(parityReportRaw)) {
  parityResult = done(`Evidence: ${parityReportRaw}`);
} else {
  parityResult = failed(`Provided parity report is missing: ${parityReportRaw}`);
}

let usageResult = pending('Usage evidence needs rollout metrics');
let usageSummary = {
  legacy_total: 0,
  store_total: 0,
  admin_total: 0,
  target_total: 0,
  legacy_share_percent: null,
  max_legacy_share_percent: maxLegacySharePercent,
};
const entrypointRows = [];

if (metricsData) {
  const entrypoints = metricsData.entrypoints;
  const legacyTotal = Object.entries(entrypoints)
    .filter(([key]) => key.startsWith('http_legacy:'))
    .reduce((sum, [, value]) => sum + value, 0);
  const storeTotal = Object.entries(entrypoints)
    .filter(([key]) => key.startsWith('http_store:'))
    .reduce((sum, [, value]) => sum + value, 0);
  const adminTotal = Object.entries(entrypoints)
    .filter(([key]) => key.startsWith('http_admin:'))
    .reduce((sum, [, value]) => sum + value, 0);
  const targetTotal = storeTotal + adminTotal;
  const total = legacyTotal + targetTotal;
  const legacySharePercent = total > 0 ? (legacyTotal / total) * 100 : null;

  usageSummary = {
    legacy_total: legacyTotal,
    store_total: storeTotal,
    admin_total: adminTotal,
    target_total: targetTotal,
    legacy_share_percent: legacySharePercent,
    max_legacy_share_percent: maxLegacySharePercent,
  };

  for (const entryPoint of ['http_legacy', 'http_store', 'http_admin']) {
    const row = {
      entry_point: entryPoint,
      library: entrypoints[`${entryPoint}:library`] ?? 0,
      core_runtime: entrypoints[`${entryPoint}:core_runtime`] ?? 0,
      bypass: entrypoints[`${entryPoint}:bypass`] ?? 0,
    };
    row.total = row.library + row.core_runtime + row.bypass;
    entrypointRows.push(row);
  }

  if (total <= 0) {
    usageResult = pending('No commerce traffic observed in metrics snapshot');
  } else if (targetTotal <= 0) {
    usageResult = pending('Only legacy ecommerce traffic is visible; target /store or /admin traffic is missing');
  } else if (legacyTotal <= 0) {
    usageResult = done('Legacy ecommerce traffic is already at zero');
  } else if (legacySharePercent !== null && legacySharePercent <= maxLegacySharePercent) {
    usageResult = done(
      `Legacy share ${legacySharePercent.toFixed(2)}% is within threshold ${maxLegacySharePercent.toFixed(2)}%`
    );
  } else {
    usageResult = pending(
      `Legacy share ${legacySharePercent.toFixed(2)}% exceeds threshold ${maxLegacySharePercent.toFixed(2)}%`
    );
  }
}

let consistencyResult = pending('Need both metrics and runtime snapshots to compare rollout policy');
const surfaceRows = [];

if (metricsData && runtimeData) {
  const mismatches = [];
  for (const surface of ['legacy', 'store', 'admin']) {
    const metricSurface = metricsData.rollout[surface];
    const runtimeSurface = runtimeData.surfaces[surface];
    const row = {
      surface,
      metrics_enabled: Boolean(metricSurface.enabled),
      runtime_enabled: Boolean(runtimeSurface.enabled),
      metrics_canary_percent: Number(metricSurface.canary_percent),
      runtime_canary_percent: Number(runtimeSurface.canary_percent),
      metrics_restricted: Boolean(metricSurface.restricted),
      runtime_restricted: Boolean(runtimeSurface.restricted),
    };
    surfaceRows.push(row);

    if (row.metrics_enabled !== row.runtime_enabled) {
      mismatches.push(`${surface}: enabled mismatch`);
    }
    if (row.metrics_canary_percent !== row.runtime_canary_percent) {
      mismatches.push(`${surface}: canary mismatch`);
    }
    if (row.metrics_restricted !== row.runtime_restricted) {
      mismatches.push(`${surface}: restricted mismatch`);
    }
  }

  if (mismatches.length > 0) {
    consistencyResult = failed(
      `Rollout policy mismatch between metrics and runtime snapshot: ${mismatches.join('; ')}`
    );
  } else {
    consistencyResult = done('Metrics and runtime snapshots expose the same rollout policy');
  }
}

let overallStatus = 'done';
for (const candidate of [
  { status: integrationStatus, note: integrationNote },
  parityResult,
  usageResult,
  metricsResult,
  runtimeResult,
  consistencyResult,
]) {
  if (statusRank(candidate.status) > statusRank(overallStatus)) {
    overallStatus = candidate.status;
  }
}

const summary = {
  generated_at_utc: timestampUtc,
  environment: envName,
  overall_status: overallStatus,
  local_regression: {
    status: integrationStatus,
    note: integrationNote,
    logs: {
      rollout_middleware: rolloutLog,
      store_openapi: openapiLog,
      legacy_openapi: legacyOpenapiLog,
      schema_smoke: schemaSmokeLog,
      startup_router: startupLog,
    },
  },
  parity_evidence: {
    status: parityResult.status,
    note: parityResult.note,
    path: parityReportRaw || null,
  },
  usage_evidence: {
    status: usageResult.status,
    note: usageResult.note,
    ...usageSummary,
    entrypoints: entrypointRows,
  },
  metrics_snapshot: {
    status: metricsResult.status,
    note: metricsResult.note,
    path: metricsPathRaw,
    source: metricsSource,
    rollout: metricsData ? metricsData.rollout : null,
  },
  runtime_snapshot: {
    status: runtimeResult.status,
    note: runtimeResult.note,
    path: runtimePathRaw,
    source: runtimeSource,
    status_value: runtimeData ? runtimeData.status : null,
    observed_status: runtimeData ? runtimeData.observed_status : null,
    rollout_mode: runtimeData ? runtimeData.rollout : null,
    reasons: runtimeData ? runtimeData.reasons : [],
    surfaces: runtimeData ? runtimeData.surfaces : null,
  },
  snapshot_consistency: {
    status: consistencyResult.status,
    note: consistencyResult.note,
    surfaces: surfaceRows,
  },
};

const lines = [];
lines.push('# Commerce rollout operator report');
lines.push('');
lines.push(`- Timestamp (UTC): ${timestampUtc}`);
lines.push(`- Environment: ${envName}`);
lines.push(`- Overall status: ${overallStatus}`);
lines.push('');
lines.push('## Summary');
lines.push('');
lines.push('| Evidence | Status | Details |');
lines.push('| --- | --- | --- |');
lines.push(`| Local regression slice | ${titleCase(integrationStatus)} | ${integrationNote} |`);
lines.push(`| Parity evidence | ${titleCase(parityResult.status)} | ${parityResult.note} |`);
lines.push(`| Legacy usage evidence | ${titleCase(usageResult.status)} | ${usageResult.note} |`);
lines.push(`| Metrics snapshot | ${titleCase(metricsResult.status)} | ${metricsResult.note} |`);
lines.push(`| Runtime snapshot | ${titleCase(runtimeResult.status)} | ${runtimeResult.note} |`);
lines.push(`| Snapshot consistency | ${titleCase(consistencyResult.status)} | ${consistencyResult.note} |`);
lines.push('');
lines.push('## Rollout Policy');
lines.push('');
lines.push('| Surface | Metrics enabled | Runtime enabled | Metrics canary % | Runtime canary % | Metrics restricted | Runtime restricted |');
lines.push('| --- | ---: | ---: | ---: | ---: | ---: | ---: |');
if (surfaceRows.length > 0) {
  for (const row of surfaceRows) {
    lines.push(
      `| ${row.surface} | ${row.metrics_enabled ? 1 : 0} | ${row.runtime_enabled ? 1 : 0} | ${row.metrics_canary_percent} | ${row.runtime_canary_percent} | ${row.metrics_restricted ? 1 : 0} | ${row.runtime_restricted ? 1 : 0} |`
    );
  }
} else {
  lines.push('| n/a | 0 | 0 | 0 | 0 | 0 | 0 |');
}

lines.push('');
lines.push('## Usage Evidence');
lines.push('');
lines.push(`- Legacy usage threshold: <= ${maxLegacySharePercent.toFixed(2)}% share of ecommerce HTTP traffic`);
if (usageSummary.legacy_share_percent === null) {
  lines.push('- Observed legacy share: n/a');
} else {
  lines.push(`- Observed legacy share: ${usageSummary.legacy_share_percent.toFixed(2)}%`);
}
lines.push('');
lines.push('| Entry point | Library | Core runtime | Bypass | Total |');
lines.push('| --- | ---: | ---: | ---: | ---: |');
if (entrypointRows.length > 0) {
  for (const row of entrypointRows) {
    lines.push(
      `| ${row.entry_point} | ${row.library.toFixed(0)} | ${row.core_runtime.toFixed(0)} | ${row.bypass.toFixed(0)} | ${row.total.toFixed(0)} |`
    );
  }
} else {
  lines.push('| n/a | 0 | 0 | 0 | 0 |');
}

lines.push('');
lines.push('## Runtime Reasons');
lines.push('');
if (summary.runtime_snapshot.reasons.length > 0) {
  for (const reason of summary.runtime_snapshot.reasons) {
    lines.push(`- ${reason}`);
  }
} else {
  lines.push('- none');
}

lines.push('');
lines.push('## Sources');
lines.push('');
lines.push(`- Metrics source: ${metricsSource}`);
lines.push(`- Runtime source: ${runtimeSource}`);
lines.push(`- Metrics snapshot file: ${metricsPathRaw}`);
lines.push(`- Runtime snapshot file: ${runtimePathRaw}`);
if (parityReportRaw) {
  lines.push(`- Parity report file: ${parityReportRaw}`);
}

lines.push('');
lines.push('## Local Artifacts');
lines.push('');
lines.push(`- rollout middleware log: ${rolloutLog}`);
lines.push(`- store OpenAPI log: ${openapiLog}`);
lines.push(`- legacy OpenAPI log: ${legacyOpenapiLog}`);
lines.push(`- migration smoke log: ${schemaSmokeLog}`);
lines.push(`- startup router log: ${startupLog}`);

process.stdout.write(lines.join('\n'));
process.stdout.write('\n__SUMMARY_JSON__\n');
process.stdout.write(JSON.stringify(summary, null, 2));
JS
)

REPORT_CONTENT="${NODE_OUTPUT%$'\n__SUMMARY_JSON__'*}"
SUMMARY_JSON="${NODE_OUTPUT##*$'\n__SUMMARY_JSON__'$'\n'}"

mkdir -p "$(dirname "$OUT_FILE")"
printf '%s\n' "$REPORT_CONTENT" > "$OUT_FILE"

mkdir -p "$(dirname "$SUMMARY_OUT")"
printf '%s\n' "$SUMMARY_JSON" > "$SUMMARY_OUT"

echo "Report: $OUT_FILE"
echo "Summary: $SUMMARY_OUT"

if [[ "$LOCAL_TEST_FAILURE" == "true" ]]; then
  echo "Local regression slice failed." >&2
  exit 1
fi
