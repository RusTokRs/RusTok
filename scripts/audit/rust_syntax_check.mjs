#!/usr/bin/env node
// Syntax-only Rust parse check using the real tree-sitter-rust grammar.
//
// Purpose: the audit sandbox has no Rust toolchain (`cargo`/`rustc` are absent), so a changed
// `.rs` file cannot be compiled or formatted locally. tree-sitter-rust is the grammar editors
// use, so a file that parses without ERROR/MISSING nodes is syntactically well-formed even
// though it is not yet type-checked. This is a guard against hand-edited Rust that would fail
// at `cargo check`; it is NOT a substitute for the crate's canonical checks.
//
// Usage: node scripts/audit/rust_syntax_check.mjs [files...]   (defaults to a git diff scan)
// Exit code: 0 when every parsed file is clean, 1 when any file has syntax errors.

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';

// The grammar lives outside the repository (it is audit tooling, not a product dependency), so
// point `AUDIT_NODE_MODULES` at the directory holding it.
const modulesRoot = process.env.AUDIT_NODE_MODULES ?? process.cwd();
const require = createRequire(join(modulesRoot, 'audit-tooling.js'));
let Parser;
let Rust;
try {
  Parser = require('tree-sitter');
  Rust = require('tree-sitter-rust');
} catch (error) {
  console.error(
    `tree-sitter is required: npm install tree-sitter tree-sitter-rust in ${modulesRoot} (${error.message})`,
  );
  process.exit(2);
}

const args = process.argv.slice(2);
const files = args.length
  ? args
  : execFileSync('git', ['diff', '--name-only', '--diff-filter=ACMR', 'HEAD'], {
      encoding: 'utf8',
    })
      .split('\n')
      .filter((path) => path.endsWith('.rs'));

if (!files.length) {
  console.log('No Rust files to parse.');
  process.exit(0);
}

const parser = new Parser();
parser.setLanguage(Rust);

let failed = 0;
for (const file of files) {
  const source = readFileSync(file, 'utf8');
  const tree = parser.parse(source);
  // Walk the tree and collect ERROR / MISSING nodes with their positions.
  const problems = [];
  const visit = (node) => {
    if (node.type === 'ERROR' || node.isMissing) {
      problems.push(
        `${node.type === 'ERROR' ? 'ERROR' : 'MISSING'} ${node.type} at ${node.startPosition.row + 1}:${node.startPosition.column + 1}`,
      );
    }
    for (const child of node.children) visit(child);
  };
  visit(tree.rootNode);
  if (problems.length) {
    failed += 1;
    console.error(`✗ ${file}`);
    for (const problem of problems.slice(0, 10)) console.error(`    ${problem}`);
  } else {
    console.log(`✓ ${file}`);
  }
}

process.exit(failed ? 1 : 0);
