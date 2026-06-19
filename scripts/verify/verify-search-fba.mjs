import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-search-fba] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const s of snippets) if (!text.includes(s)) fail(`${label} missing ${s}`); }

const registryPath = 'crates/rustok-search/contracts/search-fba-registry.json';
const evidencePath = 'crates/rustok-search/contracts/evidence/search-contract-test-static-matrix.json';
const registry = json(registryPath);
const evidence = json(evidencePath);

if (registry.schema_version !== 1) fail('registry schema_version drift');
if (registry.module !== 'search' || registry.role !== 'provider' || registry.status !== 'in_progress') fail('registry identity/status drift');
if (registry.contract_version !== 'search.query.v1') fail('contract_version drift');
const ports = registry.ports ?? [];
for (const expected of ['SearchQueryPort', 'SearchSuggestionPort']) {
  if (!ports.find((p) => p.name === expected)) fail(`missing ${expected}`);
}
for (const port of ports) {
  if (port.context !== 'rustok_api::ports::PortContext' || port.error !== 'rustok_api::ports::PortError') fail(`${port.name} context/error drift`);
  if (!Array.isArray(port.read_operations) || port.read_operations.length === 0) fail(`${port.name} lacks read operations`);
  if ((port.write_operations ?? []).length !== 0) fail(`${port.name} unexpectedly declares write operations`);
}

const manifest = read('crates/rustok-search/rustok-module.toml');
hasAll(manifest, ['[fba.provider]', 'registry = "contracts/search-fba-registry.json"', 'contract_version = "search.query.v1"'], 'manifest');
const cargo = read('crates/rustok-search/Cargo.toml');
hasAll(cargo, ['rustok-api.workspace = true'], 'Cargo.toml');
const lib = read('crates/rustok-search/src/lib.rs');
hasAll(lib, ['pub mod ports;', 'pub use ports::*;'], 'lib.rs');
const source = read('crates/rustok-search/src/ports.rs');
hasAll(source, ['pub trait SearchQueryPort', 'pub trait SearchSuggestionPort', 'impl SearchQueryPort for PgSearchEngine', 'PortContext', 'PortError', 'search_error_to_port_error'], 'ports.rs');
const queryImpl = source.slice(source.indexOf('impl SearchQueryPort for PgSearchEngine'));
if (!queryImpl.includes('context.require_deadline_semantics()?')) fail('execute_search does not require deadline semantics');
if (queryImpl.includes('context.require_write_semantics()?')) fail('execute_search unexpectedly requires write semantics');
if (!queryImpl.includes('request.locale.get_or_insert_with(|| context.locale.clone())')) fail('execute_search lacks locale context fallback');

if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail('evidence header drift');
const registryCases = registry.contract_tests.cases.map((c) => c.operation).sort().join('|');
const evidenceCases = evidence.cases.map((c) => c.operation).sort().join('|');
if (registryCases !== evidenceCases) fail('evidence case matrix drift');

const plan = read('crates/rustok-search/docs/implementation-plan.md');
hasAll(plan, ['- FBA status: `in_progress`', 'search-fba-registry.json', 'SearchQueryPort', 'search-contract-test-static-matrix.json'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `search` |', 'crates/rustok-search/contracts/search-fba-registry.json', '`phase_b_ready` | `in_progress`'], 'central registry');

console.log('[verify-search-fba] search FBA provider metadata, port semantics and static evidence are consistent');
