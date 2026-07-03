#!/usr/bin/env node
// Source-level inventory for the Loco RS exit plan.
// The guard does not require current Loco usage to be gone; it requires every
// remaining occurrence to be classified so new uncategorized usage cannot drift in.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

const SEARCH_ROOTS = ['apps', 'crates', 'scripts', 'docs'];
const ROOT_FILES = ['Cargo.toml', 'Cargo.lock'];
const NEEDLES = ['loco_rs', 'loco-rs', 'cargo loco', 'rustok_outbox::loco'];
const TEXT_EXTENSIONS = new Set([
  '.rs',
  '.toml',
  '.lock',
  '.md',
  '.mjs',
  '.js',
  '.ts',
  '.tsx',
  '.json',
  '.yml',
  '.yaml',
  '.sh',
  '.ps1',
]);
const SKIP_DIRS = new Set(['.git', 'target', 'node_modules', 'dist', 'build', '.next']);

const categoryOrder = [
  'host_runtime',
  'server_task',
  'server_seed',
  'server_schedule',
  'server_test',
  'module_controller',
  'module_graphql',
  'module_ui_adapter',
  'module_runtime_adapter',
  'outbox_adapter',
  'dependency_manifest',
  'lockfile',
  'scaffold_template',
  'verification_guard',
  'docs',
];

function toPosix(rel) {
  return rel.split(path.sep).join('/');
}

function read(rel) {
  return fs.readFileSync(path.join(root, rel), 'utf8');
}

function isTextFile(rel) {
  const name = path.basename(rel);
  return TEXT_EXTENSIONS.has(path.extname(name)) || name === 'Cargo.lock' || name === 'Cargo.toml';
}

function walk(rel, files) {
  const absolute = path.join(root, rel);
  if (!fs.existsSync(absolute)) return;
  const stat = fs.statSync(absolute);
  if (stat.isFile()) {
    if (isTextFile(rel)) files.push(rel);
    return;
  }
  for (const entry of fs.readdirSync(absolute, { withFileTypes: true })) {
    if (entry.isDirectory() && SKIP_DIRS.has(entry.name)) continue;
    const childRel = path.join(rel, entry.name);
    if (entry.isDirectory()) walk(childRel, files);
    else if (entry.isFile() && isTextFile(childRel)) files.push(childRel);
  }
}

function lineNumber(source, index) {
  let line = 1;
  for (let i = 0; i < index; i += 1) {
    if (source.charCodeAt(i) === 10) line += 1;
  }
  return line;
}

function classify(rel) {
  const p = toPosix(rel);

  if (p === 'Cargo.lock') return 'lockfile';
  if (p === 'Cargo.toml' || p.endsWith('/Cargo.toml')) return 'dependency_manifest';
  if (p.startsWith('scripts/verify/')) return 'verification_guard';
  if (p.startsWith('docs/') || p.includes('/docs/')) return 'docs';

  if (p.startsWith('apps/server/tests/')) return 'server_test';
  if (p.startsWith('apps/server/src/tasks/')) return 'server_task';
  if (p.startsWith('apps/server/src/seeds/')) return 'server_seed';
  if (p === 'apps/server/scheduler.yaml') return 'server_schedule';
  if (p === 'apps/server/build.rs') return 'host_runtime';
  if (p.startsWith('apps/server/src/')) return 'host_runtime';

  if (p.includes('/tests/') || p.endsWith('/tests.rs') || p.includes('/src/controllers/') && p.includes('/tests/')) {
    return 'server_test';
  }

  if (p === 'crates/rustok-mcp/src/alloy_scaffold.rs') return 'scaffold_template';
  if (p.startsWith('crates/rustok-outbox/')) return 'outbox_adapter';

  if (p.includes('/admin/src/') || p.includes('/storefront/src/') || p.startsWith('apps/admin/src/') || p.startsWith('apps/storefront/src/')) {
    return 'module_ui_adapter';
  }
  if (p.includes('/src/controllers/')) return 'module_controller';
  if (p.includes('/src/graphql')) return 'module_graphql';
  if (p.startsWith('crates/') && p.includes('/src/')) return 'module_runtime_adapter';

  return null;
}

const files = [];
for (const rel of SEARCH_ROOTS) walk(rel, files);
for (const rel of ROOT_FILES) {
  if (fs.existsSync(path.join(root, rel))) files.push(rel);
}

const occurrences = [];
const unclassified = [];

for (const rel of files.sort((a, b) => toPosix(a).localeCompare(toPosix(b)))) {
  const source = read(rel);
  for (const needle of NEEDLES) {
    let from = 0;
    while (true) {
      const index = source.indexOf(needle, from);
      if (index === -1) break;
      const category = classify(rel);
      const item = { rel: toPosix(rel), line: lineNumber(source, index), needle, category };
      occurrences.push(item);
      if (!category) unclassified.push(item);
      from = index + needle.length;
    }
  }
}

const byCategory = new Map(categoryOrder.map((category) => [category, 0]));
for (const item of occurrences) {
  byCategory.set(item.category ?? 'unclassified', (byCategory.get(item.category ?? 'unclassified') ?? 0) + 1);
}

console.log('Loco RS exit inventory');
console.log(`Scanned ${files.length} files for: ${NEEDLES.join(', ')}`);
console.log(`Found ${occurrences.length} occurrence(s)`);
for (const [category, count] of byCategory.entries()) {
  if (count > 0) console.log(`- ${category}: ${count}`);
}

if (unclassified.length > 0) {
  console.error('\nUnclassified Loco occurrence(s):');
  for (const item of unclassified.slice(0, 50)) {
    console.error(`✗ ${item.rel}:${item.line} ${item.needle}`);
  }
  if (unclassified.length > 50) console.error(`...and ${unclassified.length - 50} more`);
  console.error('\nClassify the path in verify-loco-inventory.mjs or remove the Loco dependency from that code path.');
  process.exit(1);
}

console.log('\nAll Loco occurrences are classified for the exit plan');
