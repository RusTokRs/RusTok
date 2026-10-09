#!/usr/bin/env node

/**
 * RusToK UI Parity Verifier
 * Enforces cross-framework consistency between design tokens, Rust/Leptos types,
 * Next.js TypeScript definitions, and Flutter codegen.
 *
 * Runs as part of CI via: npm run verify:ui:parity
 */

import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const rootDir = resolve(__dirname, '../..');

let failures = 0;

function pass(msg) {
  console.log(`  [PASS] ${msg}`);
}

function fail(msg) {
  console.error(`  [FAIL] ${msg}`);
  failures += 1;
}

console.log('=== RusToK UI Parity Verification ===');

// 1. Verify Tokens Existence and Structure
const tokensJsonPath = resolve(rootDir, 'UI/tokens/tokens.json');
const baseCssPath = resolve(rootDir, 'UI/tokens/base.css');
const flutterTokensPath = resolve(rootDir, 'UI/tokens/rustok_tokens.g.dart');

if (!existsSync(tokensJsonPath)) {
  fail(`Missing tokens specification at ${tokensJsonPath}`);
} else {
  try {
    const tokens = JSON.parse(readFileSync(tokensJsonPath, 'utf-8'));
    pass('tokens.json parsed successfully');

    // Check radii and spacing
    if (tokens.radius?.sm && tokens.radius?.md && tokens.radius?.lg) {
      pass('Radius scale verified (sm, md, lg)');
    } else {
      fail('Incomplete radius scale in tokens.json');
    }

    if (tokens.colors?.light?.primary && tokens.colors?.dark?.primary) {
      pass('Color semantic tokens verified (light/dark primary)');
    } else {
      fail('Missing semantic color tokens in tokens.json');
    }
  } catch (err) {
    fail(`Failed to parse tokens.json: ${err.message}`);
  }
}

// 2. Verify Base CSS Contract
if (!existsSync(baseCssPath)) {
  fail(`Missing base CSS contract at ${baseCssPath}`);
} else {
  const css = readFileSync(baseCssPath, 'utf-8');
  const requiredCssVars = [
    '--iu-radius-sm',
    '--iu-radius-md',
    '--iu-radius-lg',
    '--iu-primary',
    '--iu-bg',
    '--iu-fg'
  ];

  const missingVars = requiredCssVars.filter((v) => !css.includes(v));
  if (missingVars.length === 0) {
    pass(`All core CSS variables present in ${baseCssPath}`);
  } else {
    fail(`Missing CSS variables in base.css: ${missingVars.join(', ')}`);
  }
}

// 3. Verify Flutter Generated Tokens
if (!existsSync(flutterTokensPath)) {
  fail(`Missing generated Flutter tokens at ${flutterTokensPath}`);
} else {
  const dart = readFileSync(flutterTokensPath, 'utf-8');
  if (dart.includes('class RusTokRadius') && dart.includes('class RusTokColors')) {
    pass('Flutter tokens file verified (RusTokRadius, RusTokColors)');
  } else {
    fail('Flutter tokens file missing required classes');
  }
}

// 4. Verify Component Variant Parity (Button)
const rustTypesPath = resolve(rootDir, 'UI/leptos/src/types.rs');
const reactButtonPath = resolve(rootDir, 'apps/next-admin/src/shared/ui/shadcn/button.tsx');

if (existsSync(rustTypesPath) && existsSync(reactButtonPath)) {
  const rustCode = readFileSync(rustTypesPath, 'utf-8');
  const reactCode = readFileSync(reactButtonPath, 'utf-8');

  const expectedVariants = ['default', 'destructive', 'outline', 'secondary', 'ghost', 'link'];
  const missingInRust = expectedVariants.filter(
    (v) => !rustCode.toLowerCase().includes(v.toLowerCase())
  );
  const missingInReact = expectedVariants.filter(
    (v) => !reactCode.toLowerCase().includes(v.toLowerCase())
  );

  if (missingInRust.length === 0) {
    pass(`Rust ButtonVariant contains all canonical variants (${expectedVariants.join(', ')})`);
  } else {
    fail(`Rust ButtonVariant missing variants: ${missingInRust.join(', ')}`);
  }

  if (missingInReact.length === 0) {
    pass(`React Button contains all canonical variants (${expectedVariants.join(', ')})`);
  } else {
    fail(`React Button missing variants: ${missingInReact.join(', ')}`);
  }
} else {
  fail('Could not locate Rust or React button component definitions');
}

// 5. Verify Workbench Pages Exist in Both Hosts
const nextWorkbenchPage = resolve(rootDir, 'apps/next-admin/src/app/dashboard/design-system/page.tsx');
const leptosWorkbenchPage = resolve(rootDir, 'apps/admin/src/pages/design_system.rs');

if (existsSync(nextWorkbenchPage)) {
  pass('Next.js UI Workbench page verified at /dashboard/design-system');
} else {
  fail(`Missing Next.js UI Workbench page at ${nextWorkbenchPage}`);
}

if (existsSync(leptosWorkbenchPage)) {
  pass('Leptos UI Workbench page verified at apps/admin/src/pages/design_system.rs');
} else {
  fail(`Missing Leptos UI Workbench page at ${leptosWorkbenchPage}`);
}

console.log('-------------------------------------');
if (failures === 0) {
  console.log('[SUCCESS] All UI parity checks passed cleanly!');
  process.exit(0);
} else {
  console.error(`[FAILURE] ${failures} UI parity check(s) failed.`);
  process.exit(1);
}
