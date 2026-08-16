#!/usr/bin/env node
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const item = argv[i];
    if (!item.startsWith('--')) throw new Error(`Unexpected argument '${item}'`);
    const eq = item.indexOf('=');
    if (eq >= 0) out[item.slice(2, eq)] = item.slice(eq + 1);
    else {
      const key = item.slice(2);
      const next = argv[++i];
      if (next == null) throw new Error(`Missing value for --${key}`);
      out[key] = next;
    }
  }
  return out;
}

function numberOrNull(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function metric(metrics, ...names) {
  for (const name of names) if (metrics?.[name]) return metrics[name];
  return null;
}

function value(metricObject, key) {
  return numberOrNull(metricObject?.values?.[key]);
}

function parseTelemetry(path) {
  const lines = readFileSync(path, 'utf8').split('\n').filter(Boolean).map((line) => JSON.parse(line));
  const metadata = lines.find((item) => item.contract === 'rustok_vs_magento_process_metadata_v1');
  const samples = lines.filter((item) => item.contract === 'rustok_vs_magento_process_sample_v1');
  if (!metadata || samples.length < 2) throw new Error('Telemetry requires metadata and at least two samples');
  return { metadata, samples };
}

function processSeries(samples, target) {
  return samples.map((sample) => ({
    elapsed_ms: sample.elapsed_ms,
    process: sample.processes.find((item) => item.target === target),
  })).filter((item) => item.process?.ok);
}

function cpuCores(series, ticksPerSecond) {
  if (series.length < 2) return null;
  const first = series[0];
  const last = series[series.length - 1];
  const elapsedSeconds = (last.elapsed_ms - first.elapsed_ms) / 1000;
  if (!(elapsedSeconds > 0)) return null;
  const firstTicks = first.process.stat.utime_ticks + first.process.stat.stime_ticks;
  const lastTicks = last.process.stat.utime_ticks + last.process.stat.stime_ticks;
  return (lastTicks - firstTicks) / ticksPerSecond / elapsedSeconds;
}

function peak(series, selector) {
  const values = series.map(({ process }) => selector(process)).filter((item) => Number.isFinite(item));
  return values.length ? Math.max(...values) : null;
}

function totalStackPeakRss(samples) {
  let peakKib = null;
  for (const sample of samples) {
    const values = sample.processes.filter((item) => item.ok).map((item) => item.status.vm_rss_kib).filter(Number.isFinite);
    if (!values.length) continue;
    const sum = values.reduce((a, b) => a + b, 0);
    peakKib = peakKib == null ? sum : Math.max(peakKib, sum);
  }
  return peakKib;
}

function totalStackCpu(samples, ticksPerSecond) {
  const targets = new Set(samples.flatMap((sample) => sample.processes.filter((item) => item.ok).map((item) => item.target)));
  const values = [...targets].map((target) => cpuCores(processSeries(samples, target), ticksPerSecond)).filter(Number.isFinite);
  return values.length ? values.reduce((a, b) => a + b, 0) : null;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const summaryPath = resolve(String(args.summary || 'summary.json'));
  const telemetryPath = resolve(String(args.telemetry || 'telemetry.jsonl'));
  const outputPath = resolve(String(args.output || 'result.json'));
  const appTarget = String(args['app-target'] || 'app');
  const appVcpu = args['application-vcpu'] == null ? null : numberOrNull(args['application-vcpu']);
  if (!existsSync(summaryPath)) throw new Error(`Summary not found: ${summaryPath}`);
  if (!existsSync(telemetryPath)) throw new Error(`Telemetry not found: ${telemetryPath}`);
  if (existsSync(outputPath)) throw new Error(`Refusing to overwrite ${outputPath}`);

  const payload = JSON.parse(readFileSync(summaryPath, 'utf8'));
  const metrics = payload.k6?.metrics || payload.metrics || {};
  const measured = metric(metrics, 'measured_requests');
  if (!measured) throw new Error('summary.json is missing measured_requests; warm-up and measured throughput cannot be separated safely');
  const latency = metric(metrics, 'http_req_duration{phase:measure}', 'http_req_duration');
  const httpFailed = metric(metrics, 'http_req_failed{phase:measure}', 'http_req_failed');
  const validationFailed = metric(metrics, 'response_validation_failures{phase:measure}', 'response_validation_failures');
  const dropped = metric(metrics, 'dropped_iterations{scenario:measure}', 'dropped_iterations{phase:measure}', 'dropped_iterations');

  const achievedRps = value(measured, 'rate');
  const p50 = value(latency, 'med');
  const p95 = value(latency, 'p(95)');
  const p99 = value(latency, 'p(99)');
  const httpFailureRate = value(httpFailed, 'rate');
  const validationFailureRate = value(validationFailed, 'rate');
  const droppedIterations = value(dropped, 'count') ?? 0;
  if (!Number.isFinite(achievedRps) || achievedRps <= 0) throw new Error('measured_requests has no positive rate');

  const { metadata, samples } = parseTelemetry(telemetryPath);
  const ticksPerSecond = numberOrNull(metadata.clock_ticks_per_second) || 100;
  const appSeries = processSeries(samples, appTarget);
  if (appSeries.length < 2) throw new Error(`Telemetry does not contain at least two successful samples for app target '${appTarget}'`);
  const appPeakRssKib = peak(appSeries, (process) => process.status.vm_rss_kib);
  const appPeakHwmKib = peak(appSeries, (process) => process.status.vm_hwm_kib);
  const appCpuCores = cpuCores(appSeries, ticksPerSecond);
  const stackPeakRssKib = totalStackPeakRss(samples);
  const stackCpuCores = totalStackCpu(samples, ticksPerSecond);

  const sloPass = [p95, p99, httpFailureRate, validationFailureRate].every(Number.isFinite)
    && p95 < 250
    && p99 < 500
    && httpFailureRate < 0.001
    && validationFailureRate < 0.001
    && droppedIterations === 0;

  const appPeakMib = appPeakRssKib == null ? null : appPeakRssKib / 1024;
  const result = {
    contract: 'rustok_vs_magento_step_result_v1',
    source: {
      summary: summaryPath,
      telemetry: telemetryPath,
      platform: payload.metadata?.platform || null,
      operation: payload.metadata?.operation || null,
      requested_rps: payload.metadata?.requested_rps ?? null,
      search_term: payload.metadata?.search_term ?? null,
      search_expected_matches: payload.metadata?.search_expected_matches ?? null,
      product_sku: payload.metadata?.product_sku ?? null,
    },
    performance: {
      achieved_rps: achievedRps,
      p50_ms: p50,
      p95_ms: p95,
      p99_ms: p99,
      http_failure_rate: httpFailureRate,
      validation_failure_rate: validationFailureRate,
      dropped_iterations: droppedIterations,
      slo_pass: sloPass,
    },
    resources: {
      app_target: appTarget,
      app_average_cpu_cores: appCpuCores,
      app_peak_rss_mib: appPeakMib,
      app_peak_hwm_mib: appPeakHwmKib == null ? null : appPeakHwmKib / 1024,
      sampled_stack_average_cpu_cores: stackCpuCores,
      sampled_stack_peak_rss_mib: stackPeakRssKib == null ? null : stackPeakRssKib / 1024,
    },
    normalized: {
      app_rps_per_vcpu: achievedRps != null && appVcpu ? achievedRps / appVcpu : null,
      app_mib_per_1k_rps: achievedRps && appPeakMib != null ? appPeakMib / (achievedRps / 1000) : null,
      app_vcpu_limit: appVcpu,
    },
  };
  writeFileSync(outputPath, `${JSON.stringify(result, null, 2)}\n`, { encoding: 'utf8', flag: 'wx' });
  process.stdout.write(`${JSON.stringify({ output: outputPath, slo_pass: sloPass, achieved_rps: achievedRps })}\n`);
}

try { main(); } catch (error) { console.error(error.stack || String(error)); process.exitCode = 1; }
