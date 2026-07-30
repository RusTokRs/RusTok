import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-tenant-hardening-workflow] ${message}`);
  process.exit(1);
};

const workflow = read('.github/workflows/tenant-hardening.yml');

for (const marker of [
  'name: Tenant hardening',
  '"crates/rustok-tenant/**"',
  '"crates/rustok-auth/cli/**"',
  '"apps/storefront/src/shared/context/enabled_modules.rs"',
  '"apps/storefront/src/shared/context/enabled_modules_native_server_adapter.rs"',
  'focused-contract:',
  'postgres-concurrency:',
  'cargo fmt --all -- --check',
  'node scripts/verify/verify-tenant-locale-policy-migration.mjs',
  'npm run verify:tenant:fba',
  'node scripts/verify/verify-tenant-hardening-workflow.mjs',
  'cargo check -p rustok-tenant --tests',
  'cargo check -p rustok-storefront',
  'cargo test -p rustok-auth-cli oauth_create_app -- --nocapture',
  'cargo test -p rustok-server --test lifecycle_bypass_guard',
  'RUSTOK_TENANT_TEST_DATABASE_URL:',
  'postgres:17-alpine',
  'cargo test -p rustok-tenant --test tenant_ensure_concurrency_postgres -- --nocapture',
  'cargo test -p rustok-tenant --test locale_policy_concurrency_postgres -- --nocapture',
]) {
  if (!workflow.includes(marker)) {
    fail(`workflow contract is missing ${marker}`);
  }
}

if (workflow.includes('cargo test -p rustok-tenant --test tenant_ensure_concurrency_postgres --locked')) {
  fail('tenant concurrency evidence must not be hidden behind the repository Cargo.lock drift');
}

console.log(
  '[verify-tenant-hardening-workflow] focused tenant, storefront and PostgreSQL evidence workflow is retained',
);
