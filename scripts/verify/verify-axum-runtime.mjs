#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

const requiredMarkers = [
  ['apps/server/src/host.rs', 'pub async fn run'],
  ['apps/server/src/services/server_bootstrap.rs', 'ServerRuntimeContext'],
  ['apps/server/src/services/app_router.rs', 'Router'],
  ['apps/server/src/services/app_lifecycle.rs', 'ServerRuntimeContext'],
  ['crates/rustok-cli/src/main.rs', 'run_with_environment'],
  ['crates/rustok-runtime/src/lib.rs', 'RuntimeComposition'],
];

function fail(message) {
  throw new Error(`[verify-axum-runtime] ${message}`);
}

for (const [relativePath, marker] of requiredMarkers) {
  const filePath = path.join(root, relativePath);
  if (!fs.existsSync(filePath)) fail(`missing ${relativePath}`);
  const source = fs.readFileSync(filePath, 'utf8');
  if (!source.includes(marker)) fail(`${relativePath} is missing ${marker}`);
}

console.log('[verify-axum-runtime] Axum host and standalone CLI runtime boundaries are present');
