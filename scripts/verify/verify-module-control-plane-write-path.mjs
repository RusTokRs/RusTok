#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const serverRoot = path.join(root, 'apps/server/src');
const writePattern = /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+(?:platform_state|module_operations|tenant_modules|module_artifact_[a-z_]+|module_build_requests|registry_[a-z_]+)\b/i;
const activeModelPattern = /\b(?:module_operations|tenant_modules|module_artifact_[a-z_]+|module_build_requests|registry_[a-z_]+)::ActiveModel\b/;
const ownerBoundaries = [
  {
    path: 'crates/rustok-modules/src/composition.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+platform_state\b/i,
  },
  {
    path: 'crates/rustok-modules/src/operation_store.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+(?:module_operations|tenant_modules)\b/i,
  },
  {
    path: 'crates/rustok-modules/src/installation.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+module_artifact_[a-z_]+\b/i,
  },
  {
    path: 'crates/rustok-modules/src/build.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+module_build_requests\b/i,
  },
  {
    path: 'crates/rustok-modules/src/governance.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+registry_[a-z_]+\b/i,
  },
];

function fail(message) {
  throw new Error(`[verify-module-control-plane-write-path] ${message}`);
}

function rustFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) return rustFiles(entryPath);
    return entry.isFile() && entry.name.endsWith('.rs') ? [entryPath] : [];
  });
}

function relative(filePath) {
  return path.relative(root, filePath).replaceAll(path.sep, '/');
}

function writesControlPlane(source) {
  return writePattern.test(source) || activeModelPattern.test(source);
}

try {
  const violations = rustFiles(serverRoot)
    .filter((filePath) => !relative(filePath).startsWith('apps/server/src/models/'))
    .filter((filePath) => writesControlPlane(fs.readFileSync(filePath, 'utf8')))
    .map(relative);

  if (violations.length > 0) {
    fail(`control-plane writes must be owner-owned; found: ${violations.join(', ')}`);
  }

  for (const owner of ownerBoundaries) {
    const source = fs.readFileSync(path.join(root, owner.path), 'utf8');
    if (!owner.pattern.test(source)) {
      fail(`owner write implementation is missing: ${owner.path}`);
    }
  }

  console.log('[verify-module-control-plane-write-path] owner boundaries verified');
} catch (error) {
  if (
    error instanceof Error &&
    error.message.startsWith('[verify-module-control-plane-write-path]')
  ) {
    console.error(error.message);
    process.exit(1);
  }
  throw error;
}
