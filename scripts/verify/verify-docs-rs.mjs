#!/usr/bin/env node
/**
 * Verification Gate & Audit Scanner for docs.rs & Public Crate Documentation
 *
 * Scans crates in crates/modules/, crates/libs/, and crates/ui/ to verify:
 * 1. src/lib.rs contains substantive top-level crate documentation (//! or #![doc = ...])
 * 2. Absence of wildcard re-exports (pub use ...::*) in src/lib.rs
 * 3. Module separation: native modules must place RusToKModule in src/module.rs instead of src/lib.rs
 * 4. Cargo.toml defines [package.metadata.docs.rs] with all-features and rustdoc-args
 * 5. Presence of CRATE_API.md with mandatory contract and invariant sections
 * 6. README.md links to CRATE_API.md
 * 7. Verification of contract test harnesses where required
 *
 * Usage:
 *   node scripts/verify/verify-docs-rs.mjs                # Scan all crates
 *   node scripts/verify/verify-docs-rs.mjs --crate <name> # Scan a specific crate
 *   node scripts/verify/verify-docs-rs.mjs --modules      # Scan crates/modules only
 *   node scripts/verify/verify-docs-rs.mjs --libs         # Scan crates/libs only
 *   node scripts/verify/verify-docs-rs.mjs --ui           # Scan crates/ui only
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

const scanModules = args.includes('--modules') || (!args.includes('--libs') && !args.includes('--ui'));
const scanLibs = args.includes('--libs') || (!args.includes('--modules') && !args.includes('--ui'));
const scanUi = args.includes('--ui') || (!args.includes('--modules') && !args.includes('--libs'));

const searchDirs = [];
if (scanModules) searchDirs.push({ type: 'module', dir: path.join(repoRoot, 'crates/modules') });
if (scanLibs) searchDirs.push({ type: 'lib', dir: path.join(repoRoot, 'crates/libs') });
if (scanUi) searchDirs.push({ type: 'ui', dir: path.join(repoRoot, 'crates/ui') });

const results = [];

for (const { type, dir } of searchDirs) {
  if (!fs.existsSync(dir)) continue;
  const entries = fs.readdirSync(dir, { withFileTypes: true });

  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    const crateName = entry.name;
    if (targetCrate && crateName !== targetCrate) continue;

    const crateDir = path.join(dir, crateName);
    const libRsPath = path.join(crateDir, 'src/lib.rs');
    const moduleRsPath = path.join(crateDir, 'src/module.rs');
    const cargoTomlPath = path.join(crateDir, 'Cargo.toml');
    const moduleTomlPath = path.join(crateDir, 'rustok-module.toml');
    const crateApiPath = path.join(crateDir, 'CRATE_API.md');
    const readmePath = path.join(crateDir, 'README.md');
    const srcContractTestsPath = path.join(crateDir, 'src/contract_tests.rs');
    const testsContractTestsPath = path.join(crateDir, 'tests/contract_tests.rs');

    const hasCargoToml = fs.existsSync(cargoTomlPath);
    const hasLibRs = fs.existsSync(libRsPath);
    const isNativeModule = fs.existsSync(moduleTomlPath);
    const hasModuleRs = fs.existsSync(moduleRsPath);
    const hasCrateApi = fs.existsSync(crateApiPath);
    const hasReadme = fs.existsSync(readmePath);
    const hasContractTests = fs.existsSync(srcContractTestsPath) || fs.existsSync(testsContractTestsPath);

    if (!hasCargoToml && !hasLibRs) {
      // Not a Rust crate directory (e.g. docs, assets, or scripts)
      continue;
    }

    const issues = [];

    // 1. Cargo.toml Verification
    if (hasCargoToml) {
      const cargoContent = fs.readFileSync(cargoTomlPath, 'utf8');
      const hasDocsRsTable = cargoContent.includes('[package.metadata.docs.rs]');
      if (!hasDocsRsTable) {
        issues.push('Missing [package.metadata.docs.rs] table in Cargo.toml');
      } else {
        if (!cargoContent.includes('all-features = true') && !cargoContent.includes('all-features=true')) {
          issues.push('[package.metadata.docs.rs] missing `all-features = true`');
        }
        if (!cargoContent.includes('rustdoc-args = ["--cfg", "docsrs"]')) {
          issues.push('[package.metadata.docs.rs] missing `rustdoc-args = ["--cfg", "docsrs"]`');
        }
      }

      if (!/^\s*description(\.workspace)?\s*=/m.test(cargoContent)) {
        issues.push('Cargo.toml missing `description` or `description.workspace = true`');
      }
      if (!/^\s*license(\.workspace)?\s*=/m.test(cargoContent)) {
        issues.push('Cargo.toml missing `license` or `license.workspace = true`');
      }
    } else {
      issues.push('Missing Cargo.toml');
    }

    // 2. src/lib.rs Verification
    let hasTopLevelDocs = false;
    let hasWildcardExport = false;
    let definesModuleInLib = false;

    if (hasLibRs) {
      const libContent = fs.readFileSync(libRsPath, 'utf8');

      // Top-level crate documentation check
      const docCommentMatches = libContent.match(/^\s*\/\/!.*$/gm);
      const hasDocAttr = /#!\[doc\s*=/.test(libContent);
      if (hasDocAttr) {
        hasTopLevelDocs = true;
      } else if (docCommentMatches && docCommentMatches.length >= 3) {
        // Ensure not just a 1-line stub
        hasTopLevelDocs = true;
        const hasTitle = docCommentMatches.some((line) => /^\s*\/\/!\s*#\s+/.test(line));
        if (!hasTitle) {
          issues.push('Top-level `//!` docs in src/lib.rs should have an H1 title (e.g. `//! # Crate Name`)');
        }
      }

      if (!hasTopLevelDocs) {
        issues.push('Missing substantive top-level crate documentation (`//!` or `#![doc = ...]`, >= 3 lines) in src/lib.rs');
      }

      // Wildcard export check: pub use ...::* or pub use ...::{...*...}
      hasWildcardExport = /pub\s+use\s+([^;]+::\s*\*|[^;]+::\{[^}]*\*\s*\}|[^;]+::\*\s*;)/m.test(libContent);
      if (hasWildcardExport) {
        issues.push('Uses wildcard re-export (`pub use ...::*`) in src/lib.rs; explicit named exports required');
      }

      definesModuleInLib = /impl\s+RusToKModule\s+for\s+/.test(libContent);
      if (definesModuleInLib) {
        issues.push('Declares `impl RusToKModule` directly in src/lib.rs instead of src/module.rs');
      }

      if (isNativeModule) {
        if (!/pub\s+mod\s+module;/.test(libContent)) {
          issues.push('Native module src/lib.rs does not declare `pub mod module;`');
        }
        if (!/pub\s+use\s+module::/.test(libContent)) {
          issues.push('Native module src/lib.rs does not re-export module struct via `pub use module::...;`');
        }
      }
    } else {
      issues.push('Missing src/lib.rs');
    }

    // 3. Native Module src/module.rs Separation
    if (isNativeModule && !hasModuleRs && !definesModuleInLib) {
      issues.push('Native module is missing src/module.rs');
    }

    // 4. CRATE_API.md Quality & Content Invariants
    if (!hasCrateApi) {
      issues.push('Missing CRATE_API.md');
    } else {
      const crateApiContent = fs.readFileSync(crateApiPath, 'utf8');
      if (crateApiContent.trim().length < 80) {
        issues.push('CRATE_API.md is too short or empty (stub content)');
      }

      const hasPublicSurfaceSection =
        crateApiContent.includes('## Public surface') ||
        crateApiContent.includes('## Public Modules') ||
        crateApiContent.includes('## Public modules') ||
        crateApiContent.includes('## Primary Public Types') ||
        crateApiContent.includes('## Primary public types');
      if (!hasPublicSurfaceSection) {
        issues.push('CRATE_API.md missing public surface section (`## Public surface` or `## Public modules`)');
      }

      const hasInvariantsSection =
        crateApiContent.includes('## Invariants') ||
        crateApiContent.includes('## Contract invariants') ||
        crateApiContent.includes('### Domain Invariants');
      if (!hasInvariantsSection) {
        issues.push('CRATE_API.md missing invariants section (`## Invariants` or `### Domain Invariants`)');
      }

      // For domain modules (crates/modules/*), verify full Minimum Contract Set & Capability Bindings
      if (type === 'module') {
        const requiredModuleSections = [
          '## Platform Capability Bindings',
          '## Domain Collaborations',
          '## Minimum Contract Set',
          '### Input DTOs/Commands',
          '### Domain Invariants',
          '### Events / Outbox Side Effects',
          '### Errors / Failure Codes',
        ];
        for (const sec of requiredModuleSections) {
          if (!crateApiContent.includes(sec)) {
            issues.push(`CRATE_API.md missing mandatory module section: \`${sec}\``);
          }
        }
      }
    }

    // 5. README.md Linkage
    if (hasReadme) {
      const readmeContent = fs.readFileSync(readmePath, 'utf8');
      const linksToCrateApi =
        readmeContent.includes('CRATE_API.md') ||
        readmeContent.includes('CRATE_API');
      if (!linksToCrateApi) {
        issues.push('README.md does not contain a link to CRATE_API.md');
      }
    } else {
      issues.push('Missing README.md');
    }

    // 6. Contract Tests presence and registration
    if (hasCrateApi && type === 'module') {
      if (!hasContractTests) {
        issues.push('Module missing contract test harness (contract_tests.rs validating CRATE_API.md)');
      } else {
        if (fs.existsSync(srcContractTestsPath)) {
          const libContent = hasLibRs ? fs.readFileSync(libRsPath, 'utf8') : '';
          if (!libContent.includes('contract_tests')) {
            issues.push('src/contract_tests.rs exists but is not registered in src/lib.rs (missing `mod contract_tests;`)');
          }
        }
        const testFile = fs.existsSync(srcContractTestsPath) ? srcContractTestsPath : testsContractTestsPath;
        const testContent = fs.readFileSync(testFile, 'utf8');
        if (!testContent.includes('CRATE_API.md') || !testContent.includes('Minimum Contract Set')) {
          issues.push(`${path.relative(crateDir, testFile)} does not assert CRATE_API.md contract markers`);
        }
      }
    }

    results.push({
      crate: crateName,
      type,
      isNativeModule,
      compliant: issues.length === 0,
      issues,
    });
  }
}

// Print summary
console.log('='.repeat(80));
console.log(' RusToK docs.rs & Crate Documentation Readiness Audit');
console.log('='.repeat(80));

const compliantCount = results.filter((r) => r.compliant).length;
const totalCount = results.length;
const percent = totalCount > 0 ? Math.round((compliantCount / totalCount) * 100) : 100;

for (const r of results) {
  const symbol = r.compliant ? '[\x1b[32mPASS\x1b[0m]' : '[\x1b[31mFAIL\x1b[0m]';
  const typeTag = `[\x1b[36m${r.type.toUpperCase()}\x1b[0m]`;
  console.log(`${symbol} ${typeTag} ${r.crate}`);
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

if (targetCrate) {
  const found = results.find((r) => r.crate === targetCrate);
  if (!found || !found.compliant) {
    process.exit(1);
  }
} else if (compliantCount < totalCount) {
  // If running audit sweep, return non-zero when failures exist
  process.exit(1);
}
