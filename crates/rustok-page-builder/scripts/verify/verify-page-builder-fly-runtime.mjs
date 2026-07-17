import { readFile } from 'node:fs/promises';
import process from 'node:process';

const root = process.cwd();
const read = (path) => readFile(`${root}/${path}`, 'utf8');

const [cargo, adapters, service, browser] = await Promise.all([
  read('crates/rustok-page-builder/Cargo.toml'),
  read('crates/rustok-page-builder/src/adapters.rs'),
  read('crates/rustok-page-builder/src/adapters/fly_service.rs'),
  read('crates/fly-leptos/src/lib.rs')
]);

const required = [
  [cargo, 'fly = { path = "../fly" }', 'rustok-page-builder must depend on fly'],
  [adapters, 'pub struct FlyProjectInspection', 'Fly project inspection is missing'],
  [adapters, 'GrapesJsCodec::decode_value', 'Fly codec is not used'],
  [adapters, '.project\n            .pages', 'tree traversal must start from GrapesJS pages'],
  [adapters, 'component_properties', 'component property lookup is missing'],
  [service, 'impl<S, R, T> PageBuilderCapabilityService', 'Fly-backed provider is missing'],
  [service, '.tree_nodes()', 'Fly-backed provider does not expose real tree traversal'],
  [service, '.component_properties(&input.node_id)', 'provider does not validate component lookup'],
  [browser, 'pub fn hit_test_drop_targets', 'browser hit testing is missing'],
  [browser, 'pub fn auto_scroll_delta', 'browser auto-scroll policy is missing'],
  [browser, 'FLY_IFRAME_PROTOCOL', 'iframe protocol marker is missing'],
  [browser, 'last_sequence.is_none_or', 'iframe replay protection is missing']
];

const failures = required
  .filter(([source, marker]) => !source.includes(marker))
  .map(([, , message]) => message);

if (service.includes('project_data.get("nodes")')) {
  failures.push('Fly-backed provider must not traverse the obsolete root nodes key');
}
if (browser.includes('DOM order') && !browser.includes('never mutates DOM order')) {
  failures.push('browser adapter must document that DOM order is not canonical');
}

if (failures.length > 0) {
  console.error('Fly runtime verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly runtime wiring verified.');
