import { readFileSync } from 'node:fs';
const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => { console.error(`[verify-email-fba] ${message}`); process.exit(1); };
const sameSet = (actual, expected) => Array.isArray(actual) && Array.isArray(expected) && actual.length === expected.length && expected.every((item) => actual.includes(item));
const registryPath = 'crates/rustok-email/contracts/email-fba-registry.json';
const evidencePath = 'crates/rustok-email/contracts/evidence/email-contract-test-static-matrix.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const manifest = read('crates/rustok-email/rustok-module.toml');
const plan = read('crates/rustok-email/docs/implementation-plan.md');
const central = read('docs/modules/registry.md');
const pkg = json('package.json');
const lib = read('crates/rustok-email/src/lib.rs');
const cargo = read('crates/rustok-email/Cargo.toml');
const ports = read('crates/rustok-email/src/ports.rs');
if (pkg.scripts?.['verify:email:fba'] !== 'node scripts/verify/verify-email-fba.mjs && npm run verify:foundation:fba-runtime-smoke') fail('package script verify:email:fba drift');
if (pkg.scripts?.['verify:foundation:fba-runtime-smoke'] !== 'node scripts/verify/verify-foundation-fba-runtime-smoke.mjs') fail('package script verify:foundation:fba-runtime-smoke drift');
if (registry.schema_version !== 1 || registry.module !== 'email' || registry.role !== 'provider' || !['in_progress', 'boundary_ready', 'transport_verified'].includes(registry.status)) fail('registry identity/status drift');
if (registry.contract_version !== 'email.delivery.v1') fail('contract version drift');
const [port] = registry.ports ?? [];
if (!port || port.name !== 'EmailDeliveryPort' || !port.operations.includes('send_transactional_email')) fail('EmailDeliveryPort operation missing');
if (port.context !== 'rustok_api::PortContext' || port.error !== 'rustok_api::PortError') fail('context/error drift');
if (port.deadline_required !== true || port.idempotency_required !== true) fail('email delivery must keep deadline + write idempotency semantics');
if (!manifest.includes('[fba.provider]') || !manifest.includes('registry = "contracts/email-fba-registry.json"') || !manifest.includes('contract_version = "email.delivery.v1"')) fail('manifest FBA provider drift');
if (!lib.includes('pub mod ports;') || !lib.includes('pub use ports::*;')) fail('lib must export ports');
for (const marker of ['trait EmailDeliveryPort', 'impl EmailDeliveryPort for crate::EmailService', 'require_email_delivery_policy(&context)?', 'PortCallPolicy::write()', 'validate_delivery_request', 'EmailDeliveryReceipt', 'EmailProviderMode::DisabledNoop', 'PortError::invariant_violation', 'email.idempotency_required']) {
  if (!ports.includes(marker)) fail(`ports marker missing ${marker}`);
}
for (const assertion of ['disabled_provider_noop_preserved', 'template_error_not_retryable']) {
  if (!JSON.stringify(registry).includes(assertion)) fail(`registry missing ${assertion}`);
}
if (!ports.includes('Serialize, Deserialize')) fail('FBA DTOs must be serializable');
if (!plan.includes('## FFA/FBA status block') || !plan.includes(`- FBA status: \`${registry.status}\``) || !plan.includes(registryPath) || !plan.includes('EmailDeliveryPort') || !plan.includes('email-contract-test-static-matrix.json')) fail('local plan FBA evidence drift');
if (!central.includes('| `email` |') || !central.includes(registryPath) || !central.includes(`| \`email\` | none | \`not_started\` | \`${registry.status}\``)) fail('central readiness board drift');
if (registry.status === 'transport_verified' && evidence.status !== 'runtime_verified') fail('transport_verified email requires runtime_verified evidence');
if (evidence.schema_version !== 1 || evidence.module !== 'email' || !['targeted_contract_tests_added_uncompiled', 'runtime_verified'].includes(evidence.status)) fail('evidence identity drift');
if (evidence.generated_from !== registryPath || evidence.runner !== 'scripts/verify/verify-email-fba.mjs' || evidence.contract_version !== registry.contract_version) fail('evidence source/runner/version drift');
if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) fail('evidence profile drift');
const rc = registry.contract_tests.cases.find((entry) => entry.operation === 'send_transactional_email');
const ec = evidence.cases.find((entry) => entry.operation === 'send_transactional_email');
if (!rc || !ec || !['targeted_rust_tests_added_uncompiled', 'runtime_verified'].includes(ec.execution_status) || !sameSet(ec.assertions, rc.assertions)) fail('send_transactional_email evidence case drift');
if (!sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('fallback profile drift');
if (!['disabled_noop_runtime_test_added_uncompiled', 'runtime_verified'].includes(evidence.fallback_smoke.status)) fail('fallback smoke status drift');
if (!cargo.includes('[dev-dependencies]') || !cargo.includes('tokio.workspace = true')) fail('targeted async port tests require tokio dev-dependency');
for (const testName of ec.test_names ?? []) {
  if (!ports.includes(`fn ${testName}`) && !ports.includes(`async fn ${testName}`)) fail(`evidence test missing ${testName}`);
}
for (const marker of ['#[tokio::test]', 'PortActor::service("email-contract-test")', 'with_idempotency_key("email-send-a")', 'with_deadline(Duration::from_secs(3))', 'EmailDeliveryPort::send_transactional_email', 'email.template_id_empty']) {
  if (!ports.includes(marker)) fail(`targeted contract test marker missing ${marker}`);
}
console.log('[verify-email-fba] Email FBA provider metadata, port semantics and static evidence are consistent');
