#!/usr/bin/env node
import { createWriteStream, readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { resolve } from 'node:path';

function parseArgs(argv) {
  const out = { target: [] };
  for (let i = 0; i < argv.length; i += 1) {
    const item = argv[i];
    if (!item.startsWith('--')) throw new Error(`Unexpected argument '${item}'`);
    const eq = item.indexOf('=');
    let key; let value;
    if (eq >= 0) { key = item.slice(2, eq); value = item.slice(eq + 1); }
    else { key = item.slice(2); value = argv[++i]; }
    if (value == null) throw new Error(`Missing value for --${key}`);
    if (key === 'target') out.target.push(value); else out[key] = value;
  }
  return out;
}

function positiveInt(value, fallback, name) {
  if (value == null) return fallback;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`${name} must be a positive integer`);
  return parsed;
}

function parseTarget(value) {
  const split = value.lastIndexOf(':');
  if (split <= 0) throw new Error(`Target '${value}' must be NAME:PID`);
  const name = value.slice(0, split);
  const pid = positiveInt(value.slice(split + 1), null, 'pid');
  return { name, pid };
}

function parseStatus(pid) {
  const text = readFileSync(`/proc/${pid}/status`, 'utf8');
  const map = {};
  for (const line of text.split('\n')) {
    const match = /^([^:]+):\s*(.*)$/.exec(line);
    if (match) map[match[1]] = match[2];
  }
  const kb = (key) => {
    const match = /^(\d+)\s+kB$/.exec(map[key] || '');
    return match ? Number(match[1]) : null;
  };
  return {
    name: map.Name || null,
    threads: map.Threads ? Number(map.Threads) : null,
    vm_rss_kib: kb('VmRSS'),
    vm_hwm_kib: kb('VmHWM'),
    vm_size_kib: kb('VmSize'),
  };
}

function parseStat(pid) {
  const text = readFileSync(`/proc/${pid}/stat`, 'utf8');
  const close = text.lastIndexOf(')');
  const fields = text.slice(close + 2).split(' ');
  return {
    state: fields[0],
    utime_ticks: Number(fields[11]),
    stime_ticks: Number(fields[12]),
    num_threads: Number(fields[17]),
    rss_pages: Number(fields[21]),
  };
}

function sysconf(name, fallback) {
  try {
    const value = Number(execFileSync('getconf', [name], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim());
    return Number.isFinite(value) && value > 0 ? value : fallback;
  } catch {
    return fallback;
  }
}

function sample(target) {
  try {
    return { target: target.name, pid: target.pid, ok: true, status: parseStatus(target.pid), stat: parseStat(target.pid) };
  } catch (error) {
    return { target: target.name, pid: target.pid, ok: false, error: String(error.code || error.message || error) };
  }
}

async function main() {
  if (process.platform !== 'linux') throw new Error('collect-process.mjs currently requires Linux /proc');
  const args = parseArgs(process.argv.slice(2));
  const targets = args.target.map(parseTarget);
  if (targets.length === 0) throw new Error('At least one --target NAME:PID is required');
  const intervalMs = positiveInt(args['interval-ms'], 1000, 'interval-ms');
  const durationMs = positiveInt(args['duration-ms'], 180000, 'duration-ms');
  const delayMs = args['delay-ms'] == null ? 0 : positiveInt(args['delay-ms'], 0, 'delay-ms');
  const output = resolve(String(args.output || 'telemetry.jsonl'));
  const stream = createWriteStream(output, { flags: 'wx', encoding: 'utf8' });
  const clockTicksPerSecond = sysconf('CLK_TCK', 100);
  const pageSizeBytes = sysconf('PAGESIZE', 4096);
  const metadata = {
    contract: 'rustok_vs_magento_process_metadata_v1',
    timestamp: new Date().toISOString(),
    clock_ticks_per_second: clockTicksPerSecond,
    page_size_bytes: pageSizeBytes,
    interval_ms: intervalMs,
    duration_ms: durationMs,
    delay_ms: delayMs,
    targets,
  };
  stream.write(`${JSON.stringify(metadata)}\n`);
  if (delayMs > 0) await new Promise((resolveDelay) => setTimeout(resolveDelay, delayMs));
  const startedAt = Date.now();
  let samples = 0;

  while (Date.now() - startedAt <= durationMs) {
    const record = {
      contract: 'rustok_vs_magento_process_sample_v1',
      timestamp: new Date().toISOString(),
      elapsed_ms: Date.now() - startedAt,
      processes: targets.map(sample),
    };
    if (!stream.write(`${JSON.stringify(record)}\n`)) await once(stream, 'drain');
    samples += 1;
    const remaining = durationMs - (Date.now() - startedAt);
    if (remaining <= 0) break;
    await new Promise((resolveDelay) => setTimeout(resolveDelay, Math.min(intervalMs, remaining)));
  }
  stream.end();
  await once(stream, 'finish');
  process.stdout.write(`${JSON.stringify({ output, samples, interval_ms: intervalMs, duration_ms: durationMs, delay_ms: delayMs, targets })}\n`);
}

main().catch((error) => { console.error(error.stack || String(error)); process.exitCode = 1; });
