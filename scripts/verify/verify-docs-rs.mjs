#!/usr/bin/env node
/**
 * Verification Gate & Audit Scanner for docs.rs & Public Crate Documentation
 *
 * Scans crates in crates/modules/ (and optionally crates/libs/) to verify:
 * 1. src/lib.rs contains top-level crate documentation (//! or #![doc = include_str!...])
 * 2. Absence of wildcard re-exports (pub use ...::*) in src/lib.rs
 * 3. Module separation: native modules must place RusToKModule in src/module.rs instead of src/lib.rs
 * 4. Cargo.toml defines [package.metadata.docs.rs]
 * 5. Presence of CRATE_API.md public contract documentation
 *
 * Usage:
 *   node scripts/verify/verify-docs-rs.mjs                # Scan all modules
 *   node scripts/verify/verify-docs-rs.mjs --crate <name> # Scan a specific crate
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '../..');

const args = process.argv.slice(2);
let targetCrate = null;
const crateArgIdx = args.indexOf('--crate');
if (crateArgIdx !== -1 && args[crateArgIdx + 1]) {
  targetCrate = args[crateArgIdx + 1];
}

const modulesDir = path.join(repoRoot, 'crates/modules');
const entries = fs.readdirSync(modulesDir, { withFileTypes: true });

const results = [];

for (const entry of entries) {
  if (!entry.isDirectory()) continue;
  const crateName = entry.name;
  if (targetCrate && crateName !== targetCrate) continue;

  const crateDir = path.join(modulesDir, crateName);
  const libRsPath = path.join(crateDir, 'src/lib.rs');
  const moduleRsPath = path.join(crateDir, 'src/module.rs');
  const cargoTomlPath = path.join(crateDir, 'Cargo.toml');
  const moduleTomlPath = path.join(crateDir, 'rustok-module.toml');
  const crateApiPath = path.join(crateDir, 'CRATE_API.md');

  const isNativeModule = fs.existsSync(moduleTomlPath);
  const hasLibRs = fs.existsSync(libRsPath);
  const hasModuleRs = fs.existsSync(moduleRsPath);
  const hasCargoToml = fs.existsSync(cargoTomlPath);
  const hasCrateApi = fs.existsSync(crateApiPath);

  if (!hasCargoToml && !hasLibRs) {
    // Not a Rust crate directory (might be documentation or assets)
    continue;
  }

  const issues = [];

  let hasTopLevelDocs = false;
  let hasWildcardExport = false;
  let definesModuleInLib = false;

  if (hasLibRs) {
    const libContent = fs.readFileSync(libRsPath, 'utf8');
    hasTopLevelDocs = /^\s*(\/\/\!|#!\[doc\s*=)/m.test(libContent);
    hasWildcardExport = /pub\s+use\s+[a-zA-Z0-9_:]+::\*/.test(libContent);
    definesModuleInLib = /impl\s+RusToKModule\s+for\s+/.test(libContent);

    if (!hasTopLevelDocs) {
      issues.push('Missing top-level crate documentation (`//!` or `#![doc = ...]`) in src/lib.rs');
    }
    if (hasWildcardExport) {
      issues.push('Uses wildcard re-export (`pub use ...::*`) in src/lib.rs');
    }
    if (definesModuleInLib) {
      issues.push('Declares `impl RusToKModule` directly in src/lib.rs instead of src/module.rs');
    }
  } else {
    issues.push('Missing src/lib.rs');
  }

  if (isNativeModule && !hasModuleRs && !definesModuleInLib) {
    issues.push('Native module is missing src/module.rs');
  }

  let hasDocsRsMetadata = false;
  if (hasCargoToml) {
    const cargoContent = fs.readFileSync(cargoTomlPath, 'utf8');
    hasDocsRsMetadata = cargoContent.includes('[package.metadata.docs.rs]');
    if (!hasDocsRsMetadata) {
      issues.push('Missing [package.metadata.docs.rs] in Cargo.toml');
    }
  }

  if (!hasCrateApi) {
    issues.push('Missing CRATE_API.md');
  }

  results.push({
    crate: crateName,
    isNativeModule,
    hasTopLevelDocs,
    hasModuleRs: isNativeModule ? hasModuleRs : null,
    hasNoWildcards: !hasWildcardExport,
    hasDocsRsMetadata,
    hasCrateApi,
    issues,
    compliant: issues.length === 0,
  });
}

// Print summary
console.log('='.repeat(80));
console.log(' RusToK docs.rs & Crate Documentation Readiness Audit');
console.log('='.repeat(80));

const compliantCount = results.filter((r) => r.compliant).length;
const totalCount = results.length;
const percent = Math.round((compliantCount / totalCount) * 100);

for (const r of results) {
  const symbol = r.compliant ? '[\x1b[32mPASS\x1b[0m]' : '[\x1b[31mFAIL\x1b[0m]';
  console.log(`${symbol} ${r.crate}`);
  if (r.issues.length > 0) {
    for (const issue of r.issues) {
      console.log(`       - \x1b[33m${issue}\x1b[0m`);
    }
  }
}

console.log('='.repeat(80));
console.log(
  `Compliance: ${compliantCount}/${totalCount} crates compliant (${percent}%).`
);
console.log('='.repeat(80));

if (targetCrate && results.length > 0 && !results[0].compliant) {
  process.exit(1);
}
