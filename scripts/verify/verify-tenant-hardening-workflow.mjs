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
  'redis-recovery:',
  'TENANT_RUST_FILES:',
  'rustfmt --edition 2024 $TENANT_RUST_FILES',
  'crates/rustok-tenant/tests/locale_policy_concurrency_postgres.rs',
  'crates/rustok-tenant/tests/tenant_ensure_concurrency_postgres.rs',
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
  'RUSTOK_CACHE_REAL_REDIS_URL:',
  'redis:7-alpine',
  'cargo test -p rustok-server --test tenant_locale_generation_guard',
  'cargo test -p rustok-server tenant_locale_generation --lib -- --ignored --nocapture --test-threads=1',
]) {
  if (!workflow.includes(marker)) {
    fail(`workflow contract is missing ${marker}`);
  }
}

if (workflow.includes('cargo fmt --all')) {
  fail('tenant evidence must not be blocked by unrelated workspace formatting drift');
}

if (workflow.includes('cargo test -p rustok-tenant --test tenant_ensure_concurrency_postgres --locked')) {
  fail('tenant concurrency evidence must not be hidden behind the repository Cargo.lock drift');
}

console.log(
  '[verify-tenant-hardening-workflow] focused tenant, storefront, PostgreSQL and Redis evidence workflow is retained',
);
