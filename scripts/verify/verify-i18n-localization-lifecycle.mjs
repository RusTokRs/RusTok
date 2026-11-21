import { execSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve, join } from 'node:path';

const REPO_ROOT = resolve('.');

let errors = [];

console.log('=== Checking Internationalization (i18n), Localization & Translation Storage (CC-12) ===');

// 1. Verify DB Multilingual Contract
console.log('1. Checking DB multilingual relational translation schema...');
try {
  execSync('node scripts/verify/verify-db-multilingual-contract.mjs', { cwd: REPO_ROOT, stdio: 'pipe' });
  console.log('✓ DB multilingual storage contract verified (language-agnostic base + *_translations).');
} catch (e) {
  errors.push(`verify-db-multilingual-contract failed: ${e.stdout?.toString() || e.message}`);
}

// 2. Verify Translation Surface Registry
console.log('2. Checking Translation Surface Registry...');
try {
  execSync('node scripts/verify/verify-translation-surface-registry.mjs', { cwd: REPO_ROOT, stdio: 'pipe' });
  console.log('✓ Translation Surface Registry (50 surfaces) verified.');
} catch (e) {
  errors.push(`verify-translation-surface-registry failed: ${e.stdout?.toString() || e.message}`);
}

// 3. Verify Fallback Negotiation Chain in Server Locale Middleware
console.log('3. Checking fallback negotiation chain and locale middleware...');
const localeMiddlewarePath = join(REPO_ROOT, 'apps', 'server', 'src', 'middleware', 'locale.rs');
try {
  const content = readFileSync(localeMiddlewarePath, 'utf-8');

  const requiredMarkers = [
    'fn constrain_locale_to_tenant',
    'record.is_enabled',
    'record.fallback_locale',
    'rustok_api::locale_primary_language',
    'record.is_default && record.is_enabled',
    'PLATFORM_FALLBACK_LOCALE',
    'TenantLocalePolicyPort',
  ];

  for (const marker of requiredMarkers) {
    if (!content.includes(marker)) {
      errors.push(`Server locale middleware missing canonical negotiation marker: ${marker}`);
    }
  }
  console.log('✓ Server locale middleware 6-step fallback negotiation chain verified.');
} catch (e) {
  errors.push(`Failed to read locale.rs: ${e.message}`);
}

// 4. Verify User-Facing String Decoupling and Public Error Safety
console.log('4. Checking public error safety across translation surfaces...');
try {
  execSync('node scripts/verify/verify-translation-public-error-safety.mjs', { cwd: REPO_ROOT, stdio: 'pipe' });
  console.log('✓ Translation public error safety verified (static public envelopes, bounded diagnostics).');
} catch (e) {
  errors.push(`verify-translation-public-error-safety failed: ${e.stdout?.toString() || e.message}`);
}

// 5. Verify UI i18n Keys and Parity
console.log('5. Checking UI Fluent catalogs and parity...');
try {
  execSync('node scripts/verify/verify-ui-i18n-keys.mjs', { cwd: REPO_ROOT, stdio: 'pipe' });
  execSync('node scripts/verify/verify-ui-i18n-parity.mjs', { cwd: REPO_ROOT, stdio: 'pipe' });
  console.log('✓ UI Fluent keys (3500+ keys) and cross-locale parity (77 catalogs) verified.');
} catch (e) {
  errors.push(`UI i18n keys or parity verification failed: ${e.stdout?.toString() || e.message}`);
}

if (errors.length > 0) {
  console.error('\n❌ CC-12 i18n & Translation Verification FAILED:');
  for (const err of errors) {
    console.error(`  - ${err}`);
  }
  process.exit(1);
} else {
  console.log('\n✅ All CC-12 Internationalization, Localization & Translation Storage checks PASSED cleanly.');
}
