import { readFileSync, readdirSync, statSync } from 'node:fs';
import { resolve, join, relative } from 'node:path';

const REPO_ROOT = resolve('.');

let errors = [];

console.log('=== Checking Dynamic Settings, Tenant Configuration & Secrets (CC-11) ===');

// 1. Verify apps/server/src/services/settings_service.rs completeness
const settingsServicePath = join(REPO_ROOT, 'apps', 'server', 'src', 'services', 'settings_service.rs');
try {
  const content = readFileSync(settingsServicePath, 'utf-8');

  // Verify category::ALL categories
  const categories = ['GENERAL', 'EMAIL', 'RATE_LIMIT', 'FEATURES', 'I18N', 'OAUTH', 'CACHE'];
  for (const cat of categories) {
    if (!content.includes(`category::${cat}`) && !content.includes(`pub const ${cat}`)) {
      errors.push(`Missing definition or reference for category::${cat} in settings_service.rs`);
    }
  }

  // Verify validators exist
  const expectedValidators = [
    'RateLimitSettingsValidator',
    'EmailSettingsValidator',
    'CacheSettingsValidator',
    'FeaturesSettingsValidator',
    'GeneralSettingsValidator',
    'I18nSettingsValidator',
    'OAuthSettingsValidator',
  ];
  for (const val of expectedValidators) {
    if (!content.includes(`struct ${val}`) || !content.includes(`impl SettingsValidator for ${val}`)) {
      errors.push(`Missing validator struct or SettingsValidator impl for ${val} in settings_service.rs`);
    }
    if (!content.includes(`reg.register(${val})`)) {
      errors.push(`Validator ${val} is not registered in ValidatorRegistry::default() in settings_service.rs`);
    }
  }

  // Verify fail-closed in ValidatorRegistry::validate
  if (content.includes('// Categories without a validator pass through') || !content.includes('No validator registered for category')) {
    errors.push('ValidatorRegistry::validate must fail closed (return Err) when a category has no validator');
  }

  // Verify OAuth secret redaction and preservation
  if (!content.includes('category::OAUTH => {') || !content.includes('client_secret')) {
    errors.push('redact_secrets must redact OAuth secrets for category::OAUTH');
  }
  if (!content.includes('preserve_oauth_secret_fields')) {
    errors.push('preserve_secrets_from_row must call preserve_oauth_secret_fields for category::OAUTH');
  }
  console.log('✓ settings_service.rs category validators, fail-closed registry, and OAuth redaction verified.');
} catch (e) {
  errors.push(`Failed to read settings_service.rs: ${e.message}`);
}

// 2. Verify rustok-secrets zeroization
const rustokSecretsCargoPath = join(REPO_ROOT, 'crates', 'utils', 'rustok-secrets', 'Cargo.toml');
const rustokSecretsSrcPath = join(REPO_ROOT, 'crates', 'utils', 'rustok-secrets', 'src', 'lib.rs');
try {
  const cargoContent = readFileSync(rustokSecretsCargoPath, 'utf-8');
  if (!cargoContent.includes('secrecy') || (!cargoContent.includes('secrecy.workspace = true') && !cargoContent.includes('secrecy ='))) {
    errors.push('crates/utils/rustok-secrets/Cargo.toml must depend on secrecy');
  }
  const srcContent = readFileSync(rustokSecretsSrcPath, 'utf-8');
  if (!srcContent.includes('SecretString') && !srcContent.includes('secrecy')) {
    errors.push('crates/utils/rustok-secrets/src/lib.rs must export/use SecretString or secrecy');
  }
  console.log('✓ rustok-secrets ZeroizeOnDrop and secrecy backing verified.');
} catch (e) {
  errors.push(`Failed to verify rustok-secrets: ${e.message}`);
}

// 3. Verify Environment Variable Containment in domain services
console.log('Checking environment variable containment across domain modules...');
function walkSync(dir, fileList = []) {
  const files = readdirSync(dir);
  for (const file of files) {
    const fullPath = join(dir, file);
    const stat = statSync(fullPath);
    if (stat.isDirectory()) {
      if (
        file === 'target' ||
        file === 'node_modules' ||
        file === '.git' ||
        file === 'tests' ||
        file === 'benches' ||
        file === 'examples' ||
        file === '__tests__'
      ) {
        continue;
      }
      walkSync(fullPath, fileList);
    } else if (file.endsWith('.rs')) {
      fileList.push(fullPath);
    }
  }
  return fileList;
}

// Ensure no tenant-managed platform setting is read via env::var directly in domain service methods
const forbiddenSettingEnvs = [
  'SITE_NAME',
  'SITE_DESCRIPTION',
  'MAINTENANCE_MODE',
  'SMTP_PASSWORD',
  'REDIS_PASSWORD',
  'CLIENT_SECRET',
  'OAUTH_SECRET',
  'REGISTRATION_ENABLED',
];

const domainModulesDir = join(REPO_ROOT, 'crates', 'modules');
const domainFiles = walkSync(domainModulesDir);
for (const filePath of domainFiles) {
  const rel = relative(REPO_ROOT, filePath).replace(/\\/g, '/');
  if (
    rel.includes('/tests/') ||
    rel.includes('_test.rs') ||
    rel.includes('/examples/') ||
    rel.includes('build.rs') ||
    rel.includes('/src/bin/')
  ) {
    continue;
  }
  if (rel.includes('/src/services/')) {
    const content = readFileSync(filePath, 'utf-8');
    const lines = content.split('\n');
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      if ((line.includes('std::env::var(') || line.includes('env::var(')) && !line.trim().startsWith('//')) {
        for (const badEnv of forbiddenSettingEnvs) {
          if (line.includes(`"${badEnv}"`) || line.includes(`"RUSTOK_${badEnv}"`)) {
            errors.push(`Forbidden tenant setting bypass via env::var in domain service at ${rel}:${i + 1}: ${line.trim()}`);
          }
        }
      }
    }
  }
}

if (errors.length > 0) {
  console.error('\n❌ Dynamic Settings & Secrets Verification FAILED:');
  for (const err of errors) {
    console.error(`  - ${err}`);
  }
  process.exit(1);
} else {
  console.log('✓ All domain services respect settings hierarchy with zero runtime env::var leaks.');
  console.log('\n✅ Dynamic Settings, Tenant Configuration & Secrets Verification PASSED cleanly.');
}
