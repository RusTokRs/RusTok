#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const planPath = 'crates/rustok-index/docs/implementation-plan.md';
const plan = readFileSync(new URL(planPath, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-implementation-plan-cursor] ${message}`);
  process.exit(1);
};

for (const marker of [
  '- Current milestone: `M4 - Query engine v1 (source-complete query/runtime and first parity shadow; retained live evidence remains open)`',
  '- M4 retained plan/SQL snapshots: `source_complete`',
  '- M4 explicit many-link aggregate ordering: `source_complete`',
  '- M4 source-owned schema catalog and shared query runtime: `source_complete`',
  '- M4 first Social Graph privacy parity shadow: `source_complete_metrics_evidence_tooling_execution_pending`',
  '- [x] Add retained v4 plan/SQL snapshots and synchronized source guards.',
  '- [x] Add explicit many-link MIN/MAX aggregate ordering for integer, string, timestamp,',
  '- [x] Publish source-owned schemas, compose one shared query runtime, and stage the first',
  '- [ ] Add PostgreSQL/reference-engine equivalence tests and retained live evidence.',
  'uses only the caller\'s remaining deadline budget, and always returns the owner result.',
  'The owner runs formatting, Cargo checks/tests, PostgreSQL fixtures, evidence capture,',
]) {
  if (!plan.includes(marker)) fail(`${planPath} is missing ${marker}`);
}

for (const stale of [
  'Many-link ordering remains rejected until an aggregate ordering policy exists.',
  '- [ ] Add plan/SQL snapshots and PostgreSQL/reference-engine equivalence tests.',
  'server/consumer query-port composition, source schema/mutation publication,',
]) {
  if (plan.includes(stale)) fail(`${planPath} contains stale M4 wording: ${stale}`);
}

console.log('[verify-index-implementation-plan-cursor] OK');
