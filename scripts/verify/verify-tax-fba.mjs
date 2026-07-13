import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const defaultRoot = new URL('../../', import.meta.url);

export class TaxFbaVerificationError extends Error {
  constructor(message) {
    super(message);
    this.name = 'TaxFbaVerificationError';
  }
}

const read = (root, path) => readFileSync(new URL(path, root), 'utf8');
const readJson = (root, path) => JSON.parse(read(root, path));
const fail = (message) => {
  throw new TaxFbaVerificationError(message);
};
const sameSet = (actual, expected) =>
  Array.isArray(actual) &&
  Array.isArray(expected) &&
  actual.length === expected.length &&
  expected.every((item) => actual.includes(item));

export function verifyTaxFba({ root = defaultRoot } = {}) {
  const registryPath = 'crates/rustok-tax/contracts/tax-fba-registry.json';
  const evidencePath = 'crates/rustok-tax/contracts/evidence/tax-contract-test-static-matrix.json';
  const registry = readJson(root, registryPath);
  const evidence = readJson(root, evidencePath);
  const runtimeSmoke = readJson(root, registry.evidence.runtime_contract_smoke);
  const manifest = read(root, 'crates/rustok-tax/rustok-module.toml');
  const plan = read(root, 'crates/rustok-tax/docs/implementation-plan.md');
  const central = read(root, 'docs/modules/registry.md');
  const cargo = read(root, 'crates/rustok-tax/Cargo.toml');
  const libSource = read(root, 'crates/rustok-tax/src/lib.rs');
  const portSource = read(root, 'crates/rustok-tax/src/ports.rs');
  const servicesSource = read(root, 'crates/rustok-tax/src/services.rs');

  if (registry.schema_version !== 1) fail('tax registry schema_version must be 1');
  if (registry.module !== 'tax') fail('tax registry module drift');
  if (registry.role !== 'provider') fail('tax registry role must be provider');
  if (!['in_progress', 'boundary_ready'].includes(registry.status)) fail('tax registry status must be boundary_ready');
  if (registry.contract_version !== 'tax.calculation.v1') fail('tax contract version drift');
  if (!Array.isArray(registry.ports) || registry.ports.length !== 1) fail('tax registry must expose one port');

  const [port] = registry.ports;
  if (port.name !== 'TaxCalculationPort') fail('tax port name drift');
  if (!port.operations.includes('calculate_tax')) fail('tax port lacks calculate_tax operation');
  if (port.context !== 'rustok_api::ports::PortContext') fail('tax port context drift');
  if (port.error !== 'rustok_api::ports::PortError') fail('tax port error drift');
  if (port.deadline_required !== true) fail('tax port must require deadline semantics');
  if (port.idempotency_required !== false) fail('tax calculation remains read-like and must not require write idempotency');

  if (!manifest.includes('[fba.provider]')) fail('tax manifest lacks provider metadata');
  if (!manifest.includes('registry = "contracts/tax-fba-registry.json"')) fail('tax manifest registry drift');
  if (!manifest.includes('contract_version = "tax.calculation.v1"')) fail('tax manifest contract version drift');
  if (!cargo.includes('rustok-api.workspace = true')) fail('tax Cargo.toml lacks rustok-api dependency');
  if (!libSource.includes('pub mod ports;') || !libSource.includes('pub use ports::*;')) fail('tax lib.rs must export ports');
  if (!portSource.includes('trait TaxCalculationPort')) fail('tax port source lacks trait');
  if (!portSource.includes('impl TaxCalculationPort for crate::TaxService')) fail('tax port source lacks in-process TaxService impl');
  if (!portSource.includes('context.require_policy(PortCallPolicy::read())?')) fail('tax calculate_tax must enforce shared read/deadline semantics');
  if (portSource.includes('require_write_semantics()?')) fail('tax calculate_tax must not require write idempotency semantics');
  if (!portSource.includes('PortError::validation("tax.validation"')) fail('tax errors must map to typed PortError validation');
  if (!servicesSource.includes('Serialize, Deserialize')) fail('tax service DTOs must be serializable for transport-neutral ports');

  if (!plan.includes('- FBA status: `boundary_ready`')) fail('tax local plan FBA status drift');
  if (!plan.includes(registryPath)) fail('tax local plan lacks registry evidence');
  if (!plan.includes(registry.evidence.runtime_contract_smoke)) fail('tax local plan lacks runtime smoke evidence');
  if (!central.includes('| `tax` |') || !central.includes(registryPath)) fail('central readiness board lacks tax FBA evidence');
  if (!central.includes(registry.evidence.runtime_contract_smoke)) fail('central readiness board lacks tax runtime smoke evidence');

  if (evidence.schema_version !== 1) fail('tax evidence schema_version must be 1');
  if (evidence.module !== 'tax') fail('tax evidence module drift');
  if (evidence.status !== 'static_matrix_locked') fail('tax evidence status drift');
  if (evidence.generated_from !== registryPath) fail('tax evidence source drift');
  if (evidence.runner !== 'scripts/verify/verify-tax-fba.mjs') fail('tax evidence runner drift');
  if (evidence.contract_version !== registry.contract_version) fail('tax evidence contract version drift');
  if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) fail('tax evidence profile drift');
  const registryCase = registry.contract_tests.cases.find((entry) => entry.operation === 'calculate_tax');
  const evidenceCase = evidence.cases.find((entry) => entry.operation === 'calculate_tax');
  if (!registryCase || !evidenceCase) fail('tax calculate_tax contract case missing');
  if (evidenceCase.execution_status !== 'static_locked_runtime_pending') fail('tax evidence execution status drift');
  if (!sameSet(evidenceCase.assertions, registryCase.assertions)) fail('tax evidence assertion drift');
  if (!sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) {
    fail('tax fallback evidence profile drift');
  }

  if (runtimeSmoke.generated_from !== registryPath) fail('tax runtime smoke source drift');
  if (runtimeSmoke.runner !== registry.evidence.runtime_contract_smoke_runner) fail('tax runtime smoke runner drift');
  if (runtimeSmoke.status !== 'executable_no_compile') fail('tax runtime smoke status drift');
  if (runtimeSmoke.contract_version !== registry.contract_version) fail('tax runtime smoke contract version drift');
  if (!sameSet(runtimeSmoke.fallback_profiles, registry.contract_tests.fallback_smoke.profiles)) {
    fail('tax runtime smoke fallback profile drift');
  }
  if (!sameSet(runtimeSmoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes)) {
    fail('tax runtime smoke degraded mode drift');
  }
  const runtimeCase = runtimeSmoke.cases.find((entry) => entry.operation === 'calculate_tax');
  if (!runtimeCase) fail('tax runtime smoke calculate_tax case missing');
  for (const marker of ['context.require_policy(PortCallPolicy::read())?', '.calculate(request)', '.map_err(tax_error_to_port_error)']) {
    if (!runtimeCase.source_order.includes(marker)) fail(`tax runtime smoke source order missing ${marker}`);
    if (!portSource.includes(marker) && !servicesSource.includes(marker)) fail(`tax runtime source missing ${marker}`);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    verifyTaxFba();
    console.log('tax FBA registry and static contract evidence verified');
  } catch (error) {
    if (error instanceof TaxFbaVerificationError) {
      console.error(`tax FBA verification failed: ${error.message}`);
      process.exit(1);
    }
    throw error;
  }
}
