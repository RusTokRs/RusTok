import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-tenant-locale-policy-migration] ${message}`);
  process.exit(1);
};

const migration = read(
  'crates/rustok-tenant/src/migrations/m20260726_000001_enforce_tenant_locale_policy.rs',
);

for (const marker of [
  'INSERT INTO tenant_locales (',
  'SELECT\n    t.id,\n    t.id,\n    t.default_locale',
  'WHERE NOT EXISTS (',
  'WHERE tl.tenant_id = t.id',
  'policy_revision,',
]) {
  if (!migration.includes(marker)) {
    fail(`legacy locale-policy backfill is missing ${marker}`);
  }
}

for (const marker of [
  'ADD COLUMN default_tenant_guard BINARY(16)',
  'CASE WHEN is_default THEN tenant_id ELSE NULL END',
  'ADD UNIQUE INDEX uq_tenant_locales_one_default (default_tenant_guard)',
]) {
  if (!migration.includes(marker)) {
    fail(`MySQL one-default locale guard is missing ${marker}`);
  }
}

const backfillPosition = migration.indexOf('INSERT INTO tenant_locales (');
const constraintPosition = migration.indexOf(
  'ADD CONSTRAINT ck_tenant_locales_default_enabled',
);
if (
  backfillPosition < 0 ||
  constraintPosition < 0 ||
  backfillPosition >= constraintPosition
) {
  fail('legacy locale-policy backfill must run before invariant constraints');
}

console.log(
  '[verify-tenant-locale-policy-migration] legacy tenant policies are backfilled and every backend enforces at most one default locale',
);
