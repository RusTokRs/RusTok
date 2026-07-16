#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const serverRoot = path.join(root, 'apps/server/src');
const ownerPath = 'crates/rustok-modules/src/governance.rs';
const writePattern = /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+registry_(?:governance_events|validation_jobs|validation_stages|publish_requests|module_releases|module_owners)\b/i;
const activeModelPattern = /registry_(?:governance_event|validation_job|validation_stage|publish_request|module_release|module_owner)::ActiveModel/;

function fail(message) {
  throw new Error(`[verify-module-governance-write-path] ${message}`);
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

function verifiesSource(source) {
  return writePattern.test(source) || activeModelPattern.test(source);
}

try {
  const violations = rustFiles(serverRoot)
    .filter((filePath) => !relative(filePath).startsWith('apps/server/src/models/'))
    .filter((filePath) => verifiesSource(fs.readFileSync(filePath, 'utf8')))
    .map(relative);

  if (violations.length > 0) {
    fail(`registry governance writes must be owner-owned; found: ${violations.join(', ')}`);
  }

  const ownerSource = fs.readFileSync(path.join(root, ownerPath), 'utf8');
  if (!verifiesSource(ownerSource)) fail(`owner write implementation is missing: ${ownerPath}`);

  console.log('[verify-module-governance-write-path] owner boundary verified');
} catch (error) {
  if (error instanceof Error && error.message.startsWith('[verify-module-governance-write-path]')) {
    console.error(error.message);
    process.exit(1);
  }
  throw error;
}
