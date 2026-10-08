#!/usr/bin/env node
// rustfmt conformance check without a Rust toolchain.
//
// `cargo fmt --check` is one of the Fly/Page Builder CI steps ("Focused formatting"), and the
// audit sandbox has no rustfmt. @scalar/rust-fmt embeds the real rustfmt (compiled to wasm, with
// byte-identical output asserted against the native CLI and rustfmt's own corpus), so this script
// reproduces the check: format each file with the package's edition and report the files whose
// content changes.
//
// Usage: node scripts/audit/rustfmt_check.mjs [--write] <file.rs> [...]
//        Without file arguments it checks the Rust files in the working tree diff.
// Exit code: 0 when every file already matches rustfmt output, 1 otherwise.
//
// The wasm build tracks a pinned nightly while CI uses stable rustfmt; treat a diff as "needs
// looking at", not as a verdict, and never rewrite a file with this tool unattended.

import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';

const modulesRoot = process.env.AUDIT_NODE_MODULES ?? process.cwd();
const require = createRequire(join(modulesRoot, 'audit-tooling.js'));
let format;
try {
  ({ format } = require('@scalar/rust-fmt'));
} catch (error) {
  console.error(
    `@scalar/rust-fmt is required: npm install @scalar/rust-fmt in ${modulesRoot} (${error.message})`,
  );
  process.exit(2);
}

const argv = process.argv.slice(2);
const write = argv.includes('--write');
const files = argv.filter((argument) => argument !== '--write').length
  ? argv.filter((argument) => argument !== '--write')
  : execFileSync('git', ['diff', '--name-only', '--diff-filter=ACMR', 'HEAD'], {
      encoding: 'utf8',
    })
      .split('\n')
      .filter((path) => path.endsWith('.rs'));

const edition = process.env.RUSTFMT_EDITION ?? '2024';

let differing = 0;
let checked = 0;
for (const file of files) {
  let source;
  try {
    source = readFileSync(file, 'utf8');
  } catch {
    continue;
  }
  checked += 1;
  let formatted;
  try {
    formatted = await format(source, { edition, styleEdition: edition });
  } catch (error) {
    console.error(`! ${file}: rustfmt refused (${error.message})`);
    differing += 1;
    continue;
  }
  if (formatted !== source) {
    if (write) {
      writeFileSync(file, formatted);
      console.log(`✎ ${file} reformatted`);
      continue;
    }
    differing += 1;
    const before = source.split('\n');
    const after = formatted.split('\n');
    let first = 0;
    while (first < before.length && first < after.length && before[first] === after[first]) {
      first += 1;
    }
    console.error(`✗ ${file}  (first difference at line ${first + 1})`);
    console.error(`    expected: ${JSON.stringify(after[first] ?? '<eof>')}`);
    console.error(`    found:    ${JSON.stringify(before[first] ?? '<eof>')}`);
  }
}

console.log(
  `\n${checked - differing}/${checked} files already match rustfmt (edition ${edition}); ${differing} need reformatting.`,
);
process.exit(differing ? 1 : 0);
