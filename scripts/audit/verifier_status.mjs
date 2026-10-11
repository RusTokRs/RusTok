#!/usr/bin/env node
// Verifier-family health report.
//
// A guard under `scripts/verify/` only protects the repository while it actually evaluates its
// rules. Two failure modes make a guard decorative while it still exits non-zero, so nothing
// distinguishes it from a real violation: it throws before reaching its assertions (the file it
// reads never landed), or its pinned literal stopped matching a moved/wrapped source line
// (rustfmt wraps a chain, a rename lands, the shape stays correct). A third class is
// environmental: the gate shells out to a toolchain the sandbox does not have.
//
// The 2026-10-11 Forum audit measured this: 259 `verify-forum-*` gates, 77 of them passing, and
// of the 182 failures only a handful reflected a code defect — the rest were plan-document
// markers for packets that never landed, missing fixture trees, or missing toolchains. This tool
// reproduces that split mechanically so the backlog is measurable instead of folklore.
//
// Usage: node scripts/audit/verifier_status.mjs [--prefix forum-] [--json] [--fail-on-crash]
//        [--timeout 120] [repoRoot]
//
// Exit code: 0 when the report completes; the report itself never fails the tree. Non-zero on a
// usage error, or with `--fail-on-crash` when any gate throws before evaluating its rules — that
// flag is what makes the tool promotable into CI once the `crash` bucket is empty. Read `crash`
// and `structured-fail` entries before drawing conclusions: the first output line is printed
// with each gate.
//
// Deliberately not wired into CI: it is a report, not an admission gate.

import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync } from 'node:fs';
import { isAbsolute, join, resolve } from 'node:path';

// Only a *platform* failure means the gate never reached its rules. Many guards report violations by
// throwing their own `Error: <name> verification failed: <detail>` from a `fail()` helper, complete
// with a stack; counting that as a crash would bury every real finding in a bucket of noise, so the
// pattern set stays limited to runtime errors Node raises on the gate's behalf.
const CRASH_PATTERNS = [
  /^(?:\w*Error: )?(ENOENT|EISDIR|ENOTDIR|EACCES|EPERM|ERR_[A-Z_]+):/m,
  /^(Syntax|Type|Reference|Range|URI)Error:/m,
  /^Cannot find (module|package)\b/m,
  /^Unexpected end of (JSON input|input)/m,
  /^\s+at (?:node:|Object\.(?:readFileSync|writeFileSync|readdirSync)|Module\._load|new NodeError)/m,
];
const TOOLCHAIN_PATTERNS = [
  // A gate that shells out for cargo/docker/node and gets ENOENT is an environment gap, not a
  // verdict: `spawnSync cargo ENOENT` is how those failures surface.
  /spawnSync [\w.-]+ ENOENT/,
  /(?:command|binary) [\w'".-]+ not found/i,
  /\bcargo\b[^\n]*(not found|No such file|is required|requires)/i,
  /\b(docker|psql|pg_ctl|pnpm|npm|npx|playwright|chromium|node)\b[^\n]*(not found|No such file|is required|missing)/i,
  /requires (a |the )?(real )?(postgres|rust|cargo|node toolchain|docker)/i,
];
const STRUCTURED_HEADER = /\b(verification|boundary|contract|guard|evidence|proof|check)[^\n]{0,40}failed:/i;

function parseArgs(argv) {
  const options = {
    prefix: '',
    json: false,
    failOnCrash: false,
    timeout: 120_000,
    repoRoot: process.cwd(),
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--json') {
      options.json = true;
    } else if (argument === '--fail-on-crash') {
      options.failOnCrash = true;
    } else if (argument === '--prefix') {
      options.prefix = argv[(index += 1)] ?? '';
    } else if (argument === '--timeout') {
      const seconds = Number(argv[(index += 1)]);
      if (!Number.isFinite(seconds) || seconds <= 0) {
        throw new Error('--timeout expects a positive number of seconds');
      }
      options.timeout = Math.round(seconds * 1000);
    } else if (argument === '--help' || argument === '-h') {
      options.help = true;
    } else if (argument.startsWith('-')) {
      throw new Error(`unknown option ${argument}`);
    } else {
      if (options.positional) {
        throw new Error('expected at most one repository root argument');
      }
      options.positional = argument;
    }
  }
  if (options.positional) {
    options.repoRoot = isAbsolute(options.positional)
      ? options.positional
      : resolve(process.cwd(), options.positional);
  }
  return options;
}

function classify(result, output) {
  if (result.error?.code === 'ETIMEDOUT' || result.signal === 'SIGTERM') {
    return 'timeout';
  }
  if (result.status === 0) {
    return 'pass';
  }
  const firstLine = output.split('\n').find((line) => line.trim().length > 0) ?? '';
  const structured = STRUCTURED_HEADER.test(output) || /\b(?:verification|status): ok\b/i.test(output);
  const crashed = CRASH_PATTERNS.some((pattern) => pattern.test(output));
  const toolchain =
    !structured && TOOLCHAIN_PATTERNS.some((pattern) => pattern.test(output));
  if (crashed) {
    return 'crash';
  }
  if (toolchain) {
    return 'toolchain';
  }
  // Anything else that carried a message of its own (a thrown `Error: <gate> ...`, or a bulleted
  // failure list) reached the gate's rules; only a silent or unrecognised exit stays unclassified.
  const reported =
    structured ||
    /^\w*Error(?: \[[A-Z_]+\])?: \S.{12,}$/m.test(output) || // includes `AssertionError [ERR_ASSERTION]: ...`
    /^not ok \d+ - /m.test(output) || // a node:test harness that ran and failed
    /^- \S/m.test(output); // a bulleted failure list
  if (reported) {
    return 'structured-fail';
  }
  return firstLine.length > 0 ? 'other-fail' : 'silent-fail';
}

function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`verifier status: ${error.message}`);
    return 2;
  }
  if (options.help) {
    console.log(
      'usage: node scripts/audit/verifier_status.mjs [--prefix forum-] [--json] [--fail-on-crash] [--timeout 120] [repoRoot]',
    );
    return 0;
  }
  const directory = join(options.repoRoot, 'scripts', 'verify');
  if (!existsSync(directory)) {
    console.error(`verifier status: no scripts/verify under ${options.repoRoot}`);
    return 2;
  }
  const gates = readdirSync(directory)
    .filter((name) => name.endsWith('.mjs'))
    .filter((name) => (options.prefix ? name.startsWith(`verify-${options.prefix}`) : true))
    .sort();
  if (gates.length === 0) {
    console.error(`verifier status: no gates matched prefix "verify-${options.prefix}"`);
    return 2;
  }

  const buckets = new Map();
  for (const name of gates) {
    const path = join(directory, name);
    const result = spawnSync(process.execPath, [path], {
      cwd: options.repoRoot,
      encoding: 'utf8',
      timeout: options.timeout,
      maxBuffer: 32 * 1024 * 1024,
    });
    const output = `${result.stdout ?? ''}${result.stderr ?? ''}`;
    const firstLine = output.split('\n').find((line) => line.trim().length > 0)?.trim() ?? '';
    const bucket = classify(result, output);
    if (!buckets.has(bucket)) {
      buckets.set(bucket, []);
    }
    buckets.get(bucket).push({ name, firstLine });
  }

  const order = ['pass', 'structured-fail', 'crash', 'toolchain', 'timeout', 'other-fail'];
  const counts = order
    .map((bucket) => [bucket, buckets.get(bucket) ?? []])
    .filter(([, list]) => list.length > 0);

  if (options.json) {
    console.log(
      JSON.stringify(
        {
          repoRoot: options.repoRoot,
          prefix: options.prefix,
          total: gates.length,
          buckets: Object.fromEntries(counts),
        },
        null,
        2,
      ),
    );
    return 0;
  }

  console.log(
    `verifiers under ${options.repoRoot} matching "verify-${options.prefix || ''}": ${gates.length}`,
  );
  for (const [bucket, list] of counts) {
    console.log(`\n${bucket}: ${list.length}`);
    for (const entry of list) {
      const excerpt =
        bucket === 'pass' || entry.firstLine.length === 0 ? '' : ` | ${entry.firstLine.slice(0, 96)}`;
      console.log(`  ${entry.name}${excerpt}`);
    }
  }
  console.log('');
  if (options.failOnCrash && (buckets.get('crash')?.length ?? 0) > 0) {
    console.log(
      `crash: ${(buckets.get('crash') ?? []).length} verifier(s) throw before evaluating their rules`,
    );
    return 1;
  }
  return 0;
}

process.exit(main());
