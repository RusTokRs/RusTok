#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const workflowPath = '.github/workflows/index-contract-ci.yml';
const workflow = read(workflowPath);

const fail = (message) => {
  console.error(`[verify-index-contract-ci] ${message}`);
  process.exit(1);
};

for (const marker of [
  'name: Index Contract CI',
  'workflow_dispatch:',
  'pull_request:',
  'push:',
  'contents: read',
  'persist-credentials: false',
  'RUSTUP_TOOLCHAIN: "1.96.0"',
  'node scripts/verify/verify-index-query-planner.mjs',
  'node scripts/verify/verify-index-postgres-query-compiler.mjs',
  'node scripts/verify/verify-index-query-snapshots.mjs',
  'node scripts/verify/verify-index-postgres-reference-equivalence.mjs',
  'node scripts/verify/verify-index-source-schema-registry.mjs',
  'node scripts/verify/verify-index-schema-readiness.mjs',
  'node scripts/verify/verify-index-schema-supersession.mjs',
  'node scripts/verify/verify-index-schema-scoped-source-event-id.mjs',
  'node scripts/verify/verify-index-source-replay-contract.mjs',
  'node scripts/verify/verify-index-source-refresh-event.mjs',
  'node scripts/verify/verify-index-replay-locale-job-scope.mjs',
  'node scripts/verify/verify-event-contract-digest-admission.mjs',
  'node scripts/verify/verify-index-product-refresh-event-family.mjs',
  'node scripts/verify/verify-index-product-refresh-delivery.mjs',
  'node scripts/verify/verify-index-product-refresh-host-consumer.mjs',
  'node scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs',
  'node scripts/verify/verify-index-product-current-schema-promotion-evidence.mjs',
  'cargo run --locked -p rustok-events --example event_contract_digests -- --write',
  'git diff --exit-code -- crates/libs/rustok-events/contracts/event-contract-digests.json',
  'cargo check --locked -p rustok-events -p rustok-product -p rustok-index --all-targets',
  'cargo check --locked -p rustok-distribution --features mod-product --lib',
  'cargo check --locked -p rustok-distribution --features mod-product --test product_current_schema_promotion_postgres',
  'cargo check --locked -p rustok-server --no-default-features --features mod-product --lib',
  'cargo check --locked -p rustok-server --no-default-features --features mod-product --test product_index_refresh_redelivery_postgres_iggy',
  'cargo test --locked -p rustok-events --test canonical_contracts',
  'cargo test --locked -p rustok-index source_refresh_event --lib',
  'cargo test --locked -p rustok-distribution --features mod-product product_index::refresh_event::tests --lib',
  'cargo test --locked -p rustok-server --no-default-features --features mod-product product_index_refresh_worker::tests --lib',
  'name: Index Contract Gate',
]) {
  if (!workflow.includes(marker)) fail(`${workflowPath} is missing ${marker}`);
}

const jobs = `\n${workflow.split('\njobs:\n')[1] ?? ''}`;
const job = (id) => {
  const source = jobs.split(`\n  ${id}:\n`)[1];
  if (!source) fail(`${workflowPath} is missing job ${id}`);
  return source.split(/\n  [\w-]+:\n/u, 1)[0];
};
const requireInJob = (id, markers) => {
  const source = job(id);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${id} is missing ${marker}`);
  }
};

const workspaceCargo = read('Cargo.toml');
for (const dependency of ['rustok-ui-leptos', 'rustok-forms-leptos']) {
  if (!new RegExp(`^${dependency}\\s*=\\s*\\{[^\\n}]*default-features\\s*=\\s*false\\b`, 'm').test(workspaceCargo)) {
    fail(`${dependency} must not enable default SSR features in browser packages`);
  }
}

const runCheck = 'bash scripts/ci/index-check.sh';
requireInJob('canonical-digest', [
  `${runCheck} cargo run --locked -p rustok-events --example event_contract_digests -- --write`,
  `${runCheck} git diff --exit-code -- crates/libs/rustok-events/contracts/event-contract-digests.json`,
]);
requireInJob('focused-cargo', [
  `${runCheck} cargo check --locked -p rustok-index --lib`,
  'id: index_compile',
  "!cancelled() && steps.index_compile.outcome == 'success'",
  'RUSTOK_INDEX_TEST_DATABASE_URL: postgresql://postgres:postgres@127.0.0.1:5432/rustok_index_test',
  'services:',
  'image: postgres:16',
  'POSTGRES_DB: rustok_index_test',
  '5432:5432',
  'pg_isready -U postgres -d rustok_index_test',
  'cargo test --locked -p rustok-index --lib -- --test-threads=1',
  '--test drift_repair_concrete_execution_postgres_test',
  '--test drift_repair_recovery_postgres_test',
  '--test source_reconciliation_dead_letter_admission_postgres_test',
]);
for (const [path, fixture] of [
  ['services/app_router.rs', 'index_native_json_routes_enforce_request_scope_without_embedded_admin'],
  ['services/app_router.rs', 'index_standalone_relay_preserves_scope_and_exact_recovery_kind'],
  ['graphql/index_replay.rs', 'index_admin_csr_commands_validate_against_the_operator_schema'],
]) {
  requireInJob('focused-cargo', [`cargo test --locked -p rustok-server --no-default-features --features mod-product --lib ${fixture}`]);
  if (!read(`apps/server/src/${path}`).includes(`async fn ${fixture}()`)) fail(`${fixture} is not a server test`);
}
const installWasmTarget = 'rustup target add --toolchain "$RUSTUP_TOOLCHAIN" wasm32-unknown-unknown';
requireInJob('admin-profiles', [
  'profile: [ssr, hydrate, csr]',
  'toolchain: ${{ env.RUSTUP_TOOLCHAIN }}',
  installWasmTarget,
  "RUSTUP_TOOLCHAIN: ${{ matrix.profile == 'ssr' && '1.97.1' || '1.96.0' }}",
  `${runCheck} cargo test --locked -p rustok-index-admin --no-default-features --features ssr`,
  `${runCheck} cargo check --locked -p rustok-admin --bin rustok-admin --no-default-features --features ssr`,
  `${runCheck} cargo check --locked -p rustok-index-admin --lib --no-default-features --features "$ADMIN_PROFILE" --target wasm32-unknown-unknown`,
  `${runCheck} cargo tree --locked -p rustok-admin --no-default-features --features "$ADMIN_PROFILE" --target wasm32-unknown-unknown --edges features,no-dev --invert leptos`,
  `grep -Fq 'leptos feature "ssr"'`,
  `${runCheck} cargo check --locked -p rustok-admin --lib --no-default-features --features "$ADMIN_PROFILE" --target wasm32-unknown-unknown`,
]);
const adminProfiles = job('admin-profiles');
if (adminProfiles.indexOf(installWasmTarget) >= adminProfiles.indexOf('Check Index admin in isolation')) {
  fail('admin-profiles must install the WASM target before running browser checks');
}
requireInJob('gate', [
  'if: always()',
  'needs:',
  '- admin-profiles',
  'needs.admin-profiles.result',
]);

for (const forbidden of [
  'contents: write',
  'pull-requests: write',
  'persist-credentials: true',
  'git push',
  'git commit',
  'gh pr',
  'create-pull-request',
]) {
  if (workflow.includes(forbidden)) {
    fail(`${workflowPath} contains forbidden repository mutation: ${forbidden}`);
  }
}

const [pullRequestPaths, pushPaths] = workflow
  .split('  pull_request:\n')[1]
  .split('\npermissions:')[0]
  .split('  push:\n');
for (const pathMarker of [
  '"crates/libs/rustok-events/**"',
  '"crates/modules/rustok-product/**"',
  '"crates/modules/rustok-index/**"',
  '"crates/modules/rustok-social-graph/**"',
  '"crates/modules/rustok-distribution/src/lib.rs"',
  '"crates/modules/rustok-distribution/src/tests.rs"',
  '"crates/modules/rustok-distribution/src/product_index/**"',
  '"crates/modules/rustok-distribution/src/product_variant_index.rs"',
  '"crates/modules/rustok-distribution/tests/product_current_schema_promotion_postgres.rs"',
  '"Cargo.toml"',
  '"apps/server/Cargo.toml"',
  '"apps/server/src/graphql/index_replay*.rs"',
  '"apps/server/src/services/app_router.rs"',
  '"apps/server/src/services/index_reconciliation_operator.rs"',
  '"apps/server/src/services/index_replay_runtime_composition.rs"',
  '"apps/admin/Cargo.toml"',
  '"apps/admin/src/app/auth_ssr.rs"',
  '"apps/admin/src/main.rs"',
  '"apps/admin/src/shared/api/mod.rs"',
  '"apps/server/src/services/product_index_refresh_worker.rs"',
  '"apps/server/src/services/server_bootstrap.rs"',
  '"apps/server/tests/product_index_refresh_redelivery_postgres_iggy.rs"',
  '"scripts/verify/verify-index-query-planner.mjs"',
  '"scripts/verify/verify-index-postgres-query-compiler.mjs"',
  '"scripts/verify/verify-index-query-snapshots.mjs"',
  '"scripts/verify/verify-index-postgres-reference-equivalence.mjs"',
  '"scripts/verify/verify-index-source-schema-registry.mjs"',
  '"scripts/verify/verify-index-schema-readiness.mjs"',
  '"scripts/verify/verify-index-schema-supersession.mjs"',
  '"scripts/verify/verify-index-schema-scoped-source-event-id.mjs"',
  '"scripts/verify/verify-index-source-replay-contract.mjs"',
  '"scripts/verify/verify-index-source-refresh-event.mjs"',
  '"scripts/verify/verify-index-replay-locale-job-scope.mjs"',
  '"scripts/verify/verify-index-product-refresh-delivery.mjs"',
  '"scripts/verify/verify-index-product-refresh-host-consumer.mjs"',
  '"scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs"',
  '"scripts/verify/verify-index-product-current-schema-promotion-evidence.mjs"',
  '"scripts/ci/index-check.sh"',
  '"scripts/verify/verify-index-contract-ci.mjs"',
  '".github/workflows/index-product-refresh-redelivery-evidence.yml"',
  '".github/workflows/index-product-current-schema-promotion-evidence.yml"',
  '".github/workflows/index-contract-ci.yml"',
]) {
  if (!pullRequestPaths?.includes(pathMarker) || !pushPaths?.includes(pathMarker)) {
    fail(`${workflowPath} must include ${pathMarker} in pull_request and push paths`);
  }
}

console.log('[verify-index-contract-ci] focused Index contract workflow verified');
