#!/usr/bin/env node

// Guards against the failure mode that let four Fly gates rot unnoticed: a `verify-fly-*.mjs`
// script exists, is assumed to be enforcing something, but is not referenced by any workflow. A
// gate nobody runs is worse than no gate, because it still reads like coverage.
//
// Every `scripts/verify/verify-fly-*.mjs` must be invoked from at least one GitHub workflow.

import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const verifyDirectory = path.join(repositoryRoot, 'scripts/verify');
const workflowDirectory = path.join(repositoryRoot, '.github/workflows');

const failures = [];

const gates = readdirSync(verifyDirectory)
  .filter((entry) => /^verify-fly-.*\.mjs$/.test(entry))
  .sort();

if (gates.length === 0) {
  failures.push('no verify-fly-*.mjs gates were found; has the verify directory moved?');
}

const workflows = readdirSync(workflowDirectory).filter((entry) => /\.ya?ml$/.test(entry));
const workflowSources = new Map(
  workflows.map((entry) => [entry, readFileSync(path.join(workflowDirectory, entry), 'utf8')]),
);

// A gate may legitimately be enforced by another gate that imports it (the admin runtime gates
// chain `import './verify-fly-ssr-first.mjs'`), so treat importers as wiring too.
const gateSources = new Map(
  gates.map((entry) => [entry, readFileSync(path.join(verifyDirectory, entry), 'utf8')]),
);

const directlyWired = new Set();
for (const gate of gates) {
  for (const source of workflowSources.values()) {
    if (source.includes(gate)) {
      directlyWired.add(gate);
      break;
    }
  }
}

const isWired = (gate, seen = new Set()) => {
  if (directlyWired.has(gate)) return true;
  if (seen.has(gate)) return false;
  seen.add(gate);
  for (const [importer, source] of gateSources) {
    if (importer === gate) continue;
    if (source.includes(`'./${gate}'`) || source.includes(`"./${gate}"`)) {
      if (isWired(importer, seen)) return true;
    }
  }
  return false;
};

for (const gate of gates) {
  if (!isWired(gate)) {
    failures.push(
      `${gate} is never executed by any workflow in .github/workflows (nor imported by a gate that is)`,
    );
  }
}

if (failures.length > 0) {
  console.error('Fly gate wiring verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  console.error(
    '\nAdd a step that runs the gate, or delete it. An unwired gate provides no protection.',
  );
  process.exit(1);
}

console.log(`Fly gate wiring verified (${gates.length} gates, all reachable from CI).`);
