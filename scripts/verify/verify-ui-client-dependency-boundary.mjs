#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const failures = [];

function walk(directory) {
  const result = [];
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const current = path.join(directory, entry.name);
    if (entry.isDirectory()) result.push(...walk(current));
    else if (entry.isFile() && entry.name === 'Cargo.toml') result.push(current);
  }
  return result;
}

const uiManifests = walk(path.join(root, 'crates')).filter((manifest) =>
  /[\\/](admin|storefront)[\\/]Cargo\.toml$/.test(manifest),
);

for (const manifest of uiManifests) {
  const relative = path.relative(root, manifest).replace(/\\/g, '/');
  const source = fs.readFileSync(manifest, 'utf8');
  const hydrate = source.match(/^hydrate\s*=\s*\[([^\]]*)\]/m)?.[1] ?? '';
  if (/rustok-core|rustok-blog|rustok-pages/.test(hydrate)) {
    failures.push(`${relative}: hydrate feature enables a backend runtime crate`);
  }

  for (const dependency of ['rustok-core', 'rustok-blog', 'rustok-pages']) {
    const line = source.match(new RegExp(`^${dependency}\\s*=.*$`, 'm'))?.[0];
    if (line && !/optional\s*=\s*true/.test(line)) {
      failures.push(`${relative}: ${dependency} must be optional and SSR-only`);
    }
  }
}

for (const relative of [
  'crates/rustok-product/storefront/Cargo.toml',
  'crates/rustok-pricing/storefront/Cargo.toml',
]) {
  const source = fs.readFileSync(path.join(root, relative), 'utf8');
  if (/^rustok-core\s*=/m.test(source)) {
    failures.push(`${relative}: locale matching must not depend on rustok-core`);
  }
}

if (failures.length > 0) {
  console.error('UI client dependency boundary failed:');
  for (const failure of failures) console.error(`x ${failure}`);
  process.exit(1);
}

console.log(`UI client dependency boundary passed for ${uiManifests.length} UI packages`);
