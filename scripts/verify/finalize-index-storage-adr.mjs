#!/usr/bin/env node

import { createHash } from 'node:crypto';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const prefix = '[finalize-index-storage-adr]';
const placeholderPrefix = 'TODO(index-storage-decision):';
const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const requiredDecisionKeys = [
  'status',
  'decision_date',
  'owner',
  'comparison_commit',
  'comparison_sha256',
  'selected_prototype',
  'selection_rationale',
  'rejection_rationales',
  'operational_tradeoffs',
  'migration_strategy',
  'rollback_strategy',
];
const allowedDecisionKeys = new Set(['$schema', ...requiredDecisionKeys]);

const fail = (message) => {
  throw new Error(message);
};

const usage = () => {
  console.log(
    'Usage: node scripts/verify/finalize-index-storage-adr.mjs '
    + '--comparison <comparison.json> --decision <decision.json> --output <adr.md>',
  );
};

const parseArgs = () => {
  const values = new Map();
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') {
      usage();
      return null;
    }
    if (!argument.startsWith('--') || !args[index + 1] || args[index + 1].startsWith('--')) {
      fail(`unknown or incomplete argument: ${argument}`);
    }
    if (values.has(argument)) fail(`${argument} was provided more than once`);
    values.set(argument, args[++index]);
  }
  for (const argument of ['--comparison', '--decision', '--output']) {
    if (!values.has(argument)) fail(`${argument} is required`);
  }
  return {
    comparison: values.get('--comparison'),
    decision: values.get('--decision'),
    output: values.get('--output'),
  };
};

const readJsonBytes = (filename, label) => {
  if (!existsSync(filename)) fail(`missing ${label}: ${filename}`);
  const bytes = readFileSync(filename);
  try {
    return {
      bytes,
      value: JSON.parse(bytes.toString('utf8')),
      sha256: createHash('sha256').update(bytes).digest('hex'),
    };
  } catch (error) {
    fail(`invalid JSON in ${label} ${filename}: ${error.message}`);
  }
};

const requireDecisionEnvelope = (decision) => {
  if (!decision || typeof decision !== 'object' || Array.isArray(decision)) {
    fail('decision must be an object');
  }
  for (const key of requiredDecisionKeys) {
    if (!Object.hasOwn(decision, key)) fail(`decision is missing required field ${key}`);
  }
  for (const key of Object.keys(decision)) {
    if (!allowedDecisionKeys.has(key)) fail(`decision contains unsupported field ${key}`);
  }
  if (Object.hasOwn(decision, '$schema')
      && decision.$schema !== './storage-decision.schema.json') {
    fail('decision.$schema must reference ./storage-decision.schema.json when present');
  }
};

const requireDecisionText = (value, label) => {
  if (typeof value !== 'string' || value.trim().length === 0) return;
  if (value.trim().startsWith(placeholderPrefix)) {
    fail(`${label} still contains a preparation placeholder`);
  }
};

const rejectPlaceholders = (decision) => {
  for (const field of [
    'selection_rationale',
    'operational_tradeoffs',
    'migration_strategy',
    'rollback_strategy',
  ]) {
    requireDecisionText(decision[field], `decision.${field}`);
  }
  if (decision.rejection_rationales
      && typeof decision.rejection_rationales === 'object'
      && !Array.isArray(decision.rejection_rationales)) {
    for (const [prototype, rationale] of Object.entries(decision.rejection_rationales)) {
      requireDecisionText(rationale, `decision.rejection_rationales.${prototype}`);
    }
  }
};

const insertDecisionDigest = (markdown, decisionSha256) => {
  const marker = /^- Comparison SHA-256: `([0-9a-f]{64})`$/gmu;
  const matches = [...markdown.matchAll(marker)];
  if (matches.length !== 1) {
    fail('rendered ADR must contain exactly one Comparison SHA-256 line');
  }
  if (/^- Decision SHA-256:/mu.test(markdown)) {
    fail('rendered ADR already contains a Decision SHA-256 line');
  }
  const line = matches[0][0];
  return markdown.replace(line, `${line}\n- Decision SHA-256: \`${decisionSha256}\``);
};

const main = () => {
  const args = parseArgs();
  if (args === null) return;
  const resolvedOutput = path.resolve(args.output);
  for (const [label, filename] of [['comparison', args.comparison], ['decision', args.decision]]) {
    if (resolvedOutput === path.resolve(filename)) fail(`--output must not overwrite the ${label} input`);
  }

  const comparison = readJsonBytes(args.comparison, 'comparison');
  const decision = readJsonBytes(args.decision, 'decision');
  requireDecisionEnvelope(decision.value);
  rejectPlaceholders(decision.value);

  const temporaryRoot = mkdtempSync(path.join(tmpdir(), 'rustok-index-storage-adr-'));
  try {
    const comparisonPath = path.join(temporaryRoot, 'comparison.json');
    const decisionPath = path.join(temporaryRoot, 'decision.json');
    const renderedPath = path.join(temporaryRoot, 'adr.md');
    writeFileSync(comparisonPath, comparison.bytes);
    writeFileSync(decisionPath, decision.bytes);

    const result = spawnSync(process.execPath, [
      path.join(scriptDirectory, 'render-index-storage-adr.mjs'),
      '--comparison', comparisonPath,
      '--decision', decisionPath,
      '--output', renderedPath,
    ], { encoding: 'utf8' });
    if (result.error) fail(`failed to start strict ADR renderer: ${result.error.message}`);
    if (result.signal) fail(`strict ADR renderer terminated by signal ${result.signal}`);
    if (result.status !== 0) {
      const detail = result.stderr?.trim() || result.stdout?.trim() || `exit status ${result.status}`;
      fail(`strict ADR renderer failed: ${detail}`);
    }

    const markdown = insertDecisionDigest(readFileSync(renderedPath, 'utf8'), decision.sha256);
    const parent = path.dirname(args.output);
    if (parent && parent !== '.') mkdirSync(parent, { recursive: true });
    const stagedOutput = `${args.output}.tmp-${process.pid}`;
    try {
      writeFileSync(stagedOutput, markdown, 'utf8');
      renameSync(stagedOutput, args.output);
    } finally {
      if (existsSync(stagedOutput)) rmSync(stagedOutput, { force: true });
    }
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }

  console.log(`${prefix} wrote ${args.output}`);
};

try {
  main();
} catch (error) {
  console.error(`${prefix} ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
