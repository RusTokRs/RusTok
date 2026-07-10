import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../', import.meta.url);

class CustomerFbaNoCompileVerificationError extends Error {
  constructor(message) {
    super(message);
    this.name = 'CustomerFbaNoCompileVerificationError';
  }
}

const read = (path) => readFileSync(new URL(path, root), 'utf8');
const readJson = (path) => JSON.parse(read(path));
const fail = (message) => { throw new CustomerFbaNoCompileVerificationError(message); };
const sameSet = (actual, expected) => Array.isArray(actual) && Array.isArray(expected) && actual.length === expected.length && expected.every((item) => actual.includes(item));

export function verifyCustomerFbaNoCompile() {
  const registryPath = 'crates/rustok-customer/contracts/customer-fba-registry.json';
  const staticEvidencePath = 'crates/rustok-customer/contracts/evidence/customer-contract-test-static-matrix.json';
  const runtimeSmokePath = 'crates/rustok-customer/contracts/evidence/customer-read-projection-runtime-smoke.json';
  const planPath = 'crates/rustok-customer/docs/implementation-plan.md';
  const registry = readJson(registryPath);
  const staticEvidence = readJson(staticEvidencePath);
  const runtimeSmoke = readJson(runtimeSmokePath);
  const portSource = read('crates/rustok-customer/src/ports.rs');
  const cargo = read('crates/rustok-customer/Cargo.toml');
  const manifest = read('crates/rustok-customer/rustok-module.toml');
  const plan = read(planPath);
  const readme = read('crates/rustok-customer/README.md');
  const localDocs = read('crates/rustok-customer/docs/README.md');
  const centralRegistry = read('docs/modules/registry.md');

  if (registry.schema_version !== 1) fail('customer registry schema_version must be 1');
  if (registry.module !== 'customer') fail('customer registry module drift');
  if (registry.role !== 'provider') fail('customer registry role must be provider');
  if (registry.status !== 'in_progress') fail('customer registry status must remain in_progress');
  if (registry.contract_version !== 'customer.read_projection.v1') fail('customer contract version drift');
  if (!cargo.includes('rustok-api.workspace = true')) fail('customer Cargo.toml must depend on rustok-api');
  if (!manifest.includes('registry = "contracts/customer-fba-registry.json"')) fail('customer manifest registry drift');
  if (!manifest.includes('contract_version = "customer.read_projection.v1"')) fail('customer manifest contract version drift');
  if (!centralRegistry.includes('| `customer` |') || !centralRegistry.includes(registryPath)) fail('central readiness board must reference customer FBA registry');
  if (!portSource.includes('trait CustomerReadPort')) fail('CustomerReadPort trait missing');
  if (!portSource.includes('require_policy(PortCallPolicy::read())?')) fail('CustomerReadPort must enforce read policy');

  for (const operation of ['read_customer_projection', 'list_customer_projections']) {
    if (!portSource.includes(`${operation}(`)) fail(`CustomerReadPort missing ${operation}`);
    if (!registry.ports?.[0]?.operations?.includes(operation)) fail(`registry missing ${operation}`);
    if (!staticEvidence.cases?.some((entry) => entry.operation === operation)) fail(`static evidence missing ${operation}`);
    if (!runtimeSmoke.covered_operations?.includes(operation)) fail(`runtime smoke missing ${operation}`);
  }

  if (staticEvidence.status !== 'static_matrix_locked') fail('static evidence status drift');
  if (staticEvidence.generated_from !== registryPath) fail('static evidence generated_from drift');
  if (staticEvidence.contract_version !== registry.contract_version) fail('static evidence contract version drift');
  if (!sameSet(staticEvidence.profiles, registry.contract_tests.profiles)) fail('static evidence profile drift');
  if (staticEvidence.promotion_gate !== 'does_not_raise_boundary_ready_without_runtime_execution') fail('static evidence must keep promotion gated');
  if (runtimeSmoke.status !== 'source_locked_live_runtime_pending') fail('runtime smoke status drift');
  if (runtimeSmoke.promotion_allowed !== false) fail('runtime smoke must block promotion');
  if (runtimeSmoke.source_tests !== 'crates/rustok-customer/tests/customer_service_test.rs') fail('runtime smoke source tests drift');
  for (const expectedCode of ['port.deadline_required', 'customer.tenant_id_invalid', 'customer.customer_not_found']) {
    if (!JSON.stringify(runtimeSmoke.typed_error_matrix).includes(expectedCode)) fail(`runtime smoke missing typed error ${expectedCode}`);
  }

  for (const doc of [plan, readme, localDocs]) {
    if (!doc.includes('node scripts/verify/verify-customer-fba-no-compile.mjs')) fail('customer docs must reference the no-compile customer gate');
  }
  if (!plan.includes('- FBA status: `in_progress`')) fail('plan FBA status drift');
  if (!plan.includes('Local documentation is synchronized')) fail('plan must record synchronized local documentation');
  if (!plan.includes('no-compile')) fail('plan must record the active no-compile verification gate');
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    verifyCustomerFbaNoCompile();
    console.log('customer FBA no-compile source/evidence gate verified');
  } catch (error) {
    if (error instanceof CustomerFbaNoCompileVerificationError) {
      console.error(`customer FBA no-compile verification failed: ${error.message}`);
      process.exit(1);
    }
    throw error;
  }
}
