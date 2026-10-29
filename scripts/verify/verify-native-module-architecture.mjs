#!/usr/bin/env node
// Strict, opt-in architecture preflight for one native server module.
// Static evidence only: it never certifies compilation, FFA parity or FBA runtime.
import { existsSync, lstatSync, readFileSync, readdirSync } from 'node:fs';
import { basename, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const SLOTS = new Set([
  'domain', 'dto', 'entities', 'services', 'ports', 'integrations', 'graphql',
  'controllers', 'rest', 'migrations', 'tests', 'events', 'error',
]);
const UI_SLOTS = ['core', 'transport', 'ui', 'tests'];
const DOMAIN_IMPORT = /\b(?:sea_orm|sea_orm_migration|axum|async_graphql|leptos|dioxus|rustok_web)\s*::/;
const SERVICE_IMPORT = /\b(?:axum|async_graphql|leptos|dioxus|rustok_web)\s*::/;
const CORE_IMPORT = /\b(?:leptos|dioxus|leptos_router|leptos_axum|web_sys|wasm_bindgen)\s*::/;
const SLUG = /^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$/;

function isFile(path) {
  return existsSync(path) && lstatSync(path).isFile();
}
function isDir(path) {
  return existsSync(path) && lstatSync(path).isDirectory();
}
function read(path) {
  return readFileSync(path, 'utf8');
}
function field(toml, section, key) {
  const header = new RegExp('^\\[' + section.replaceAll('.', '\\.') + '\\][ \\t]*$', 'm');
  const match = header.exec(toml);
  if (!match) return null;
  const rest = toml.slice(match.index + match[0].length).split(/^\[[^\n]+\][ \t]*$/m)[0];
  const value = new RegExp('^\\s*' + key + '\\s*=\\s*"([^"]+)"', 'm').exec(rest);
  return value ? value[1] : null;
}
function rustFiles(directory, failures) {
  if (!isDir(directory)) return [];
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    const path = join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      failures.push(path + ': symlinks are forbidden in strict source trees');
    } else if (entry.isDirectory()) {
      files.push(...rustFiles(path, failures));
    } else if (entry.isFile() && entry.name.endsWith('.rs')) {
      files.push(path);
    }
  }
  return files;
}
function layer(files, pattern, label, errors) {
  for (const file of files) {
    const source = read(file).split('\n').filter(line => !/^\s*(?:\/\/|\*)/.test(line)).join('\n');
    if (pattern.test(source)) errors.push(file + ': ' + label + ' must not import forbidden runtime types');
  }
}
function size(files, limit, errors) {
  for (const file of files) {
    const bytes = lstatSync(file).size;
    if (bytes > limit) errors.push(file + ': ' + bytes + ' bytes exceeds strict ' + limit + '-byte limit');
  }
}
function checkUi(root, name, errors) {
  const pkg = join(root, name);
  const src = join(pkg, 'src');
  if (!isDir(src)) {
    errors.push(pkg + ': UI package lacks src/');
    return;
  }
  if (!isFile(join(pkg, 'Cargo.toml'))) errors.push(pkg + ': missing Cargo.toml');
  if (!isFile(join(src, 'lib.rs'))) errors.push(src + ': missing UI facade lib.rs');
  for (const slot of UI_SLOTS) {
    if (!isDir(join(src, slot))) errors.push(src + ': missing FFA ' + slot + '/');
  }
  if (!isFile(join(src, 'i18n.rs')) && !isFile(join(src, 'i18n', 'mod.rs'))) {
    errors.push(src + ': missing module-owned i18n entrypoint');
  }
  if (!isFile(join(src, 'tests', 'contract.rs'))) errors.push(src + ': missing tests/contract.rs');
  if (isFile(join(src, 'contract_tests.rs'))) errors.push(src + ': root contract_tests.rs is forbidden');
  if (isFile(join(src, 'lib.rs')) && !/\bmod\s+tests\s*;/.test(read(join(src, 'lib.rs')))) {
    errors.push(src + '/lib.rs: missing tests module wiring');
  }
  const transport = join(src, 'transport');
  const wire = isFile(join(transport, 'mod.rs')) ? read(join(transport, 'mod.rs')) : '';
  if (!isFile(join(transport, 'graphql_adapter.rs')) ||
    !isFile(join(transport, 'native_server_adapter.rs')) ||
    !/\bmod\s+graphql_adapter\s*;/.test(wire) ||
    !/\bmod\s+native_server_adapter\s*;/.test(wire)) {
    errors.push(src + ': FFA must wire both GraphQL and native transport adapters');
  }
  const files = rustFiles(src, errors);
  size(files, 40 * 1024, errors);
  layer(files.filter(file => file.startsWith(join(src, 'core') + sep)), CORE_IMPORT, 'FFA core', errors);
  for (const file of files.filter(file => file.startsWith(join(src, 'ui') + sep))) {
    const code = read(file);
    if (/\b(?:graphql_adapter|native_server_adapter)\s*::|\brustok_graphql\s*::|#\s*\[\s*server\b/.test(code)) {
      errors.push(file + ': UI must use the module transport facade');
    }
    if (/\bsample_rows\b|\b(?:edit|create|delete) action triggered\b/.test(code)) {
      errors.push(file + ': synthetic data or log-only CRUD is forbidden');
    }
  }
}

/** An explicit one-module preflight; never silently scans or enrolls the workspace. */
export function validateNativeModule(moduleRoot) {
  const root = resolve(moduleRoot);
  const errors = [];
  if (!isDir(root)) return [root + ': native module directory does not exist'];
  const manifest = join(root, 'rustok-module.toml');
  const cargo = join(root, 'Cargo.toml');
  const src = join(root, 'src');
  if (!isFile(manifest)) errors.push(manifest + ': required native module manifest missing');
  if (!isFile(cargo)) errors.push(cargo + ': required native Cargo.toml missing');
  if (!isDir(src)) errors.push(src + ': required src/ missing');
  if (errors.length) return errors;
  const metadata = read(manifest);
  const slug = field(metadata, 'module', 'slug');
  const pkg = field(read(cargo), 'package', 'name');
  if (!slug || !SLUG.test(slug)) errors.push(manifest + ': module.slug must be canonical snake_case');
  if (slug && pkg !== 'rustok-' + slug.replaceAll('_', '-')) {
    errors.push(cargo + ': package.name does not match module.slug');
  }
  if (!isFile(join(src, 'lib.rs'))) errors.push(src + ': missing lib.rs');
  if (!isFile(join(src, 'module.rs'))) errors.push(src + ': missing module.rs');
  if (!isFile(join(src, 'tests', 'mod.rs')) || !isFile(join(src, 'tests', 'contract.rs'))) {
    errors.push(src + ': new native modules require tests/mod.rs and tests/contract.rs');
  }
  for (const entry of readdirSync(src, { withFileTypes: true })) {
    const path = join(src, entry.name);
    if (entry.isSymbolicLink()) errors.push(path + ': symlinks are forbidden');
    else if (entry.isFile() && entry.name.endsWith('.rs') && !['lib.rs', 'module.rs'].includes(entry.name)) {
      errors.push(path + ': strict backend src/ root admits only lib.rs and module.rs');
    } else if (entry.isDirectory() && !SLOTS.has(entry.name)) {
      errors.push(path + ': unrecognized backend responsibility slot');
    }
  }
  const lib = isFile(join(src, 'lib.rs')) ? read(join(src, 'lib.rs')) : '';
  const moduleSource = isFile(join(src, 'module.rs')) ? read(join(src, 'module.rs')) : '';
  if (!/\bmod\s+module\s*;/.test(lib)) errors.push(src + '/lib.rs: must declare module.rs');
  if (!/\bmod\s+tests\s*;/.test(lib)) errors.push(src + '/lib.rs: must wire the contract tests');
  if (!/\bimpl\s+RusToKModule\s+for\s+\w+/.test(moduleSource)) {
    errors.push(src + '/module.rs: missing RusToKModule implementation');
  }
  if (/\bimpl\s+(?:RusToKModule|MigrationSource)\s+for\b|\bregister_(?:runtime_extensions|event_listeners)\s*\(/.test(lib)) {
    errors.push(src + '/lib.rs: runtime composition belongs to module.rs');
  }
  if (/\bpub\s+(?:\(crate\)\s*)?mod\s+entities\s*;|\bpub\s+use\s+(?:crate::)?entities\s*::/.test(lib)) {
    errors.push(src + '/lib.rs: persistence entities must remain private');
  }
  const entities = isDir(join(src, 'entities'));
  const migrations = isDir(join(src, 'migrations'));
  if (entities !== migrations) errors.push(src + ': owned entities and migrations must be paired');
  if (migrations && !/\bimpl\s+MigrationSource\s+for\s+\w+/.test(moduleSource)) {
    errors.push(src + '/module.rs: missing MigrationSource implementation');
  }
  for (const slot of SLOTS) {
    const path = join(src, slot);
    if (!isDir(path)) continue;
    const mod = join(path, 'mod.rs');
    if (!isFile(mod)) errors.push(path + ': missing mod.rs');
    const contents = rustFiles(path, errors);
    if (contents.length < 2 && isFile(mod)) {
      const code = read(mod).replace(/\/\/[^\n]*/g, '');
      if (!/\b(?:pub|async|fn|struct|enum|trait|impl|type|const|static|use|mod)\b/.test(code)) {
        errors.push(path + ': empty responsibility slot is forbidden');
      }
    }
  }
  const files = rustFiles(src, errors);
  size(files, 32 * 1024, errors);
  layer(files.filter(file => file.startsWith(join(src, 'domain') + sep)), DOMAIN_IMPORT, 'domain', errors);
  layer(files.filter(file => file.startsWith(join(src, 'services') + sep)), SERVICE_IMPORT, 'services', errors);

  const mode = field(metadata, 'module', 'ui_classification');
  const expected = {
    no_ui: [], admin_only: ['admin'], storefront_only: ['storefront'],
    dual_surface: ['admin', 'storefront'],
  }[mode];
  if (!expected) errors.push(manifest + ': unknown module.ui_classification');
  for (const name of ['admin', 'storefront']) {
    const present = isDir(join(root, name));
    if (expected && expected.includes(name) && !present) errors.push(root + ': declared ' + name + ' UI missing');
    if (expected && !expected.includes(name) && present) errors.push(root + ': undeclared ' + name + ' UI present');
    const registered = field(metadata, 'provides.' + name + '_ui', 'leptos_crate') !== null;
    if (present !== registered) errors.push(manifest + ': [provides.' + name + '_ui] registration mismatch');
    if (present) checkUi(root, name, errors);
  }
  return errors;
}

function cli(args) {
  if (args.length !== 2 || !['--module', '--module-root'].includes(args[0])) {
    console.error('Usage: node scripts/verify/verify-native-module-architecture.mjs --module <slug> | --module-root <path>');
    process.exitCode = 2;
    return;
  }
  if (args[0] === '--module' && !SLUG.test(args[1])) {
    console.error('Invalid native module slug (canonical snake_case required)');
    process.exitCode = 2;
    return;
  }
  const repo = resolve(fileURLToPath(new URL('../..', import.meta.url)));
  const root = args[0] === '--module'
    ? join(repo, 'crates', 'modules', 'rustok-' + args[1].replaceAll('_', '-'))
    : resolve(args[1]);
  const errors = validateNativeModule(root);
  if (errors.length) {
    console.error('Native module architecture: FAIL (' + errors.length + ' violation(s))');
    for (const error of errors) console.error('- ' + error);
    process.exitCode = 1;
  } else {
    console.log('Native module architecture: PASS (' + basename(root) + '; static architecture only; no FBA/FFA runtime verification)');
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  cli(process.argv.slice(2));
}
