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
const sameSet = (actual, expected) => Array.isArray(actual) && actual.length === expected.length && expected.every((item) => actual.includes(item));

export function verifyCustomerFbaNoCompile() {
  const registryPath = 'crates/rustok-customer/contracts/customer-fba-registry.json';
  const staticEvidencePath = 'crates/rustok-customer/contracts/evidence/customer-contract-test-static-matrix.json';
  const runtimeSmokePath = 'crates/rustok-customer/contracts/evidence/customer-read-projection-runtime-smoke.json';
  const planPath = 'crates/rustok-customer/docs/implementation-plan.md';
  const registry = readJson(registryPath);
  const staticEvidence = readJson(staticEvidencePath);
  const runtimeSmoke = readJson(runtimeSmokePath);
  const libSource = read('crates/rustok-customer/src/lib.rs');
  const portSource = read('crates/rustok-customer/src/ports.rs');
  const wrapperSource = read('crates/rustok-customer/src/read_context.rs');
  const cargo = read('crates/rustok-customer/Cargo.toml');
  const manifest = read('crates/rustok-customer/rustok-module.toml');
  const plan = read(planPath);
  const readme = read('crates/rustok-customer/README.md');
  const localDocs = read('crates/rustok-customer/docs/README.md');
  const centralRegistry = read('docs/modules/registry.md');
  const commerceCustomerConsumers = [
    'crates/rustok-commerce/src/graphql/mutations/helpers.rs',
    'crates/rustok-commerce/src/graphql/query.rs',
    'crates/rustok-commerce/src/controllers/store/mod.rs',
    'crates/rustok-commerce/src/controllers/store/orders.rs',
    'crates/rustok-commerce/src/storefront_checkout_runtime.rs',
  ].map(read);

  if (registry.schema_version !== 1) fail('customer registry schema_version must be 1');
  if (registry.module !== 'customer') fail('customer registry module drift');
  if (registry.role !== 'provider') fail('customer registry role must be provider');
  if (!['in_progress', 'boundary_ready'].includes(registry.status)) fail('customer registry status must remain boundary_ready');
  if (registry.contract_version !== 'customer.read_projection.v1') fail('customer contract version drift');
  if (!cargo.includes('rustok-api.workspace = true')) fail('customer Cargo.toml must depend on rustok-api');
  if (!manifest.includes('registry = "contracts/customer-fba-registry.json"')) fail('customer manifest registry drift');
  if (!manifest.includes('contract_version = "customer.read_projection.v1"')) fail('customer manifest contract version drift');
  if (!centralRegistry.includes('| `customer` |') || !centralRegistry.includes(registryPath)) fail('central readiness board must reference customer FBA registry');
  if (!portSource.includes('trait CustomerReadPort')) fail('CustomerReadPort trait missing');
  if (!portSource.includes('require_customer_read_policy(&context, owner_operation)?;')) fail('CustomerReadPort operations must use the shared read-policy helper');
  if (!portSource.includes('context\n        .require_policy(PortCallPolicy::read())')) fail('CustomerReadPort helper must enforce read policy');
  if (!libSource.includes('pub mod ports;') || !libSource.includes('mod read_context;')) fail('customer crate must retain ports and compose the root context wrapper');
  if (!libSource.includes('pub use ports::{') || !libSource.includes('CustomerReadPort,')) fail('customer crate must export read contracts from ports');
  if (!libSource.includes('pub use read_context::{InProcessCustomerReadPort, in_process_customer_read_port};')) fail('customer root factory must use the context wrapper');
  if (!wrapperSource.includes('impl CustomerReadPort for InProcessCustomerReadPort')) fail('customer root wrapper must implement CustomerReadPort');
  if (!wrapperSource.includes('CustomerReadPort::read_customer_projection(&self.inner, context, request).await')) fail('customer wrapper must delegate customer-id reads');
  if (!wrapperSource.includes('CustomerReadPort::read_customer_projection_by_user(&self.inner, context, request).await')) fail('customer wrapper must delegate user-id reads');
  if (!wrapperSource.includes('CustomerReadPort::list_customer_projections(&self.inner, context, request).await')) fail('customer wrapper must delegate list reads');
  if (!wrapperSource.includes('CustomerReadPort::list_profile_enrichment(&self.inner, context, request).await')) fail('customer wrapper must delegate profile enrichment reads');

  for (const operation of ['read_customer_projection', 'read_customer_projection_by_user', 'list_customer_projections', 'list_profile_enrichment']) {
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
  for (const expectedCode of ['port.deadline_required', 'customer.context_invalid', 'customer.customer_not_found']) {
    if (!JSON.stringify(runtimeSmoke.typed_error_matrix).includes(expectedCode)) fail(`runtime smoke missing typed error ${expectedCode}`);
  }
  if (JSON.stringify(runtimeSmoke.typed_error_matrix).includes('customer.tenant_id_invalid')) fail('runtime smoke retains superseded customer tenant error code');

  for (const doc of [plan, readme, localDocs]) {
    if (!doc.includes('node scripts/verify/verify-customer-fba-no-compile.mjs')) fail('customer docs must reference the no-compile customer gate');
  }
  if (!plan.includes('- FBA status: `boundary_ready`')) fail('plan FBA status drift');
  if (!plan.includes('Local documentation is synchronized')) fail('plan must record synchronized local documentation');
  if (!plan.includes('no-compile')) fail('plan must record the active no-compile verification gate');
  if (!plan.includes('InProcessCustomerReadPort')) fail('plan must record canonical customer root construction');

  for (const consumer of commerceCustomerConsumers) {
    if (consumer.includes('CustomerService::new') || consumer.includes('use rustok_customer::CustomerService')) {
      fail('commerce customer consumers must not construct CustomerService directly');
    }
  }
  if (!commerceCustomerConsumers.some((consumer) => consumer.includes('read_customer_projection_by_user('))) {
    fail('commerce customer consumers must invoke CustomerReadPort user projection');
  }
  if (!commerceCustomerConsumers.some((consumer) => consumer.includes('in_process_customer_read_port'))) {
    fail('commerce customer consumers must compose the canonical root customer factory');
  }
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
