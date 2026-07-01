#!/usr/bin/env node
// No-compile API surface contract guardrail for the platform verification plan.
// It verifies that GraphQL/REST optional module wiring stays manifest-driven and
// that the documented API plan points at this source-level evidence.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const failures = [];
const passes = [];

function read(rel) {
  return fs.readFileSync(path.join(root, rel), 'utf8');
}

function exists(rel) {
  return fs.existsSync(path.join(root, rel));
}

function pass(message) {
  passes.push(message);
}

function fail(message) {
  failures.push(message);
}

function walk(rel, predicate) {
  const result = [];
  const visit = (currentRel) => {
    for (const entry of fs.readdirSync(path.join(root, currentRel), { withFileTypes: true })) {
      const childRel = path.join(currentRel, entry.name);
      if (entry.isDirectory()) visit(childRel);
      else if (entry.isFile() && predicate(childRel)) result.push(childRel);
    }
  };
  visit(rel);
  return result;
}

function requireContains(rel, needle, message) {
  const content = read(rel);
  if (content.includes(needle)) pass(message);
  else fail(`${message} (${rel} missing ${JSON.stringify(needle)})`);
}

function requireNotContains(rel, needle, message) {
  const content = read(rel);
  if (!content.includes(needle)) pass(message);
  else fail(`${message} (${rel} unexpectedly contains ${JSON.stringify(needle)})`);
}

function moduleEntries() {
  const raw = read('modules.toml');
  const entries = [];
  const moduleRe = /^([A-Za-z0-9_]+)\s*=\s*\{([^\n]+)\}/gm;
  let match;
  while ((match = moduleRe.exec(raw))) {
    const slug = match[1];
    const body = match[2];
    const crateMatch = body.match(/crate\s*=\s*"([^"]+)"/);
    const pathMatch = body.match(/path\s*=\s*"([^"]+)"/);
    const required = /required\s*=\s*true/.test(body);
    if (crateMatch && pathMatch) {
      entries.push({ slug, crateName: crateMatch[1], modulePath: pathMatch[1], required });
    }
  }
  return entries;
}

function tableContains(rel, slug) {
  return read(rel).includes(`| \`${slug}\``) || read(rel).includes(`\`${slug}\``);
}

function inspectPackageManifest(entry) {
  const manifestRel = `${entry.modulePath}/rustok-module.toml`;
  if (!exists(manifestRel)) return null;
  const raw = read(manifestRel);
  return {
    rel: manifestRel,
    raw,
    hasGraphql: raw.includes('[provides.graphql]'),
    hasHttp: raw.includes('[provides.http]'),
    hasEntryType: /\[crate\][\s\S]*entry_type\s*=\s*"[^"]+"/.test(raw),
    slugMatches: new RegExp(`\[module\][\\s\\S]*slug\\s*=\\s*"${entry.slug}"`).test(raw),
  };
}

function isTestRel(rel) {
  return rel.includes(`${path.sep}tests${path.sep}`) || rel.endsWith(`${path.sep}tests.rs`);
}

function isInsideTestModule(source, index) {
  const before = source.slice(0, index);
  const lastCfgTest = before.lastIndexOf('#[cfg(test)]');
  if (lastCfgTest !== -1 && /#\[cfg\(test\)\]\s*mod\s+\w+/m.test(source.slice(lastCfgTest, index))) {
    return true;
  }
  const testModuleMatches = [...before.matchAll(/(?:^|\n)\s*(?:#\[cfg\(test\)\]\s*)?mod\s+\w*tests\b/g)];
  const lastTestModule =
    testModuleMatches.length > 0 ? testModuleMatches[testModuleMatches.length - 1].index ?? -1 : -1;
  if (lastTestModule === -1) return false;
  const lastNonTestModule = before.lastIndexOf('\nmod ');
  return lastTestModule >= lastNonTestModule;
}

function shouldForbidSystemAuthority(rel) {
  return (
    rel.endsWith(`${path.sep}graphql${path.sep}query.rs`) ||
    rel.includes(`${path.sep}storefront${path.sep}`) ||
    rel.includes(`${path.sep}controllers${path.sep}`) ||
    rel.endsWith('ports.rs') ||
    rel.endsWith('services.rs')
  );
}

// GraphQL composition root must use generated optional modules instead of a hardcoded list.
requireContains('apps/server/src/graphql/schema.rs', 'schema_codegen::OptionalModuleQuery', 'GraphQL Query includes generated optional module root');
requireContains('apps/server/src/graphql/schema.rs', 'schema_codegen::OptionalModuleMutation', 'GraphQL Mutation includes generated optional module root');
requireContains('apps/server/src/graphql/schema.rs', 'include!(concat!(env!("OUT_DIR"), "/graphql_schema_codegen.rs"))', 'GraphQL schema uses build-time generated code');

// Build script must read both the platform module manifest and package-local transport declarations.
requireContains('apps/server/build.rs', 'RUSTOK_MODULES_MANIFEST', 'Build script supports explicit module manifest path');
requireContains('apps/server/build.rs', 'apply_module_package_manifest', 'Build script imports module-local rustok-module transport declarations');
requireContains('apps/server/build.rs', 'render_graphql_codegen', 'Build script renders optional GraphQL composition');
requireContains('apps/server/build.rs', 'render_routes_codegen', 'Build script renders optional REST route mounting');
requireContains('apps/server/build.rs', 'provides.graphql', 'Build script schema understands [provides.graphql]');
requireContains('apps/server/build.rs', 'provides.http', 'Build script schema understands [provides.http]');

// API plan/docs must expose this guardrail as no-compile evidence.
requireContains('docs/verification/platform-api-surfaces-verification-plan.md', 'verify-api-surface-contract.mjs', 'API verification plan lists no-compile source guardrail');
requireContains('scripts/verify/README.md', 'verify-api-surface-contract.mjs', 'Verification README documents API surface guardrail');

// Neutral Port* contracts belong to rustok-api without pulling runtime crates
// into its default dependency graph.
requireContains('crates/rustok-api/src/ports.rs', 'pub struct PortContext', 'rustok-api owns PortContext');
requireContains('crates/rustok-api/src/ports.rs', 'pub struct PortError', 'rustok-api owns PortError');
requireNotContains('crates/rustok-api/src/ports.rs', 'rustok_core', 'rustok-api Port contracts do not re-export core');
requireContains('crates/rustok-api/src/permissions.rs', 'pub struct Permission', 'rustok-api owns Permission');
requireContains('crates/rustok-api/src/permissions.rs', 'pub enum Action', 'rustok-api owns Action');
requireContains('crates/rustok-api/src/permissions.rs', 'pub enum Resource', 'rustok-api owns Resource');
requireContains('crates/rustok-api/src/locale.rs', 'pub const PLATFORM_FALLBACK_LOCALE', 'rustok-api owns platform fallback locale');
requireContains('crates/rustok-api/src/locale.rs', 'pub fn extract_locale_tag_from_header', 'rustok-api owns Accept-Language parsing');
requireNotContains('crates/rustok-api/Cargo.toml', 'rustok-core', 'rustok-api never depends on rustok-core');
for (const rel of walk('crates/rustok-api/src', (file) => file.endsWith('.rs'))) {
  requireNotContains(rel, 'rustok_core', `rustok-api source is core-independent: ${rel}`);
}
requireNotContains('crates/rustok-api/Cargo.toml', 'rustok-outbox', 'rustok-api does not depend on outbox runtime');
requireNotContains('crates/rustok-api/Cargo.toml', 'loco-rs', 'rustok-api default manifest does not own Loco runtime');
requireNotContains('crates/rustok-core/src/lib.rs', 'pub mod ports;', 'rustok-core does not publish a ports module');
requireNotContains('crates/rustok-core/src/lib.rs', 'PortContext', 'rustok-core does not re-export Port contracts');
requireNotContains('crates/rustok-core/src/lib.rs', 'pub mod permissions;', 'rustok-core does not publish permission contracts');
requireNotContains('crates/rustok-core/src/lib.rs', 'pub mod locale;', 'rustok-core does not publish locale contracts');
requireContains('crates/rustok-core/Cargo.toml', 'rustok-api.workspace = true', 'rustok-core depends on rustok-api contracts');
if (exists('crates/rustok-core/src/ports.rs')) fail('rustok-core ports implementation must be deleted');
else pass('rustok-core ports implementation is absent');
for (const rel of ['crates/rustok-core/src/permissions.rs', 'crates/rustok-core/src/locale.rs']) {
  if (exists(rel)) fail(`${rel} must be deleted after API ownership cutover`);
  else pass(`${rel} is absent`);
}

const rustSources = [...walk('apps', (file) => file.endsWith('.rs')), ...walk('crates', (file) => file.endsWith('.rs'))];
for (const rel of rustSources) {
  const source = read(rel);
  if (/rustok_core::(?:permissions(?:::|\b)|Permission\b|Action\b|Resource\b)/.test(source)) {
    fail(`${rel} uses a removed rustok-core permission path`);
  }
  if (/rustok_core::(?:locale(?:::|\b)|build_locale_candidates\b|locale_tags_match\b|normalize_locale_tag\b|PLATFORM_FALLBACK_LOCALE\b)/.test(source)) {
    fail(`${rel} uses a removed rustok-core locale path`);
  }
  if (shouldForbidSystemAuthority(rel) && source.includes('SecurityContext::system()')) {
    const matches = [...source.matchAll(/SecurityContext::system\(\)/g)];
    for (const match of matches) {
      if (!isTestRel(rel) && !isInsideTestModule(source, match.index ?? 0)) {
        fail(`${rel} grants system authority outside trusted runtime/test code`);
        break;
      }
    }
  }
  if (!isTestRel(rel) && /[A-Za-z0-9_]+_or_system\b/.test(source)) {
    fail(`${rel} exposes an *_or_system authority helper`);
  }
  if (rel !== path.join('crates', 'rustok-api', 'src', 'locale.rs')) {
    if (/(?:^|\n)\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+locale_tags_match\s*\(/.test(source)) {
      fail(`${rel} defines a package-local locale_tags_match helper`);
    }
    if (/(?:^|\n)\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+normalize_locale_tag\s*\(/.test(source)) {
      fail(`${rel} defines a package-local normalize_locale_tag helper`);
    }
  }
}
requireNotContains('crates/rustok-api/src/ui.rs', 'fn normalize_locale_tag(', 'rustok-api UI consumes canonical locale helpers');
if (exists('crates/rustok-seo-admin-support/src/locale.rs')) fail('SEO admin support locale duplicate must be deleted');
else pass('SEO admin support locale duplicate is absent');
requireContains('crates/rustok-outbox/src/ports.rs', 'use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};', 'outbox consumes canonical rustok-api Port contracts');
requireContains('crates/rustok-outbox/Cargo.toml', 'rustok-api.workspace = true', 'outbox depends on the neutral API contract layer');
requireContains('crates/rustok-outbox/src/lib.rs', 'pub mod loco;', 'outbox owns its Loco composition adapter');
requireContains('DECISIONS/2026-07-01-port-contract-ownership-and-runtime-feature-boundary.md', 'Status: Accepted', 'Port contract ownership ADR is accepted');

const entries = moduleEntries();
if (entries.length === 0) fail('modules.toml exposes module entries');
else pass(`modules.toml exposes ${entries.length} module entries`);

const optionalEntries = entries.filter((entry) => !entry.required);
if (optionalEntries.length === 0) fail('modules.toml exposes optional modules for generated API composition');
else pass(`modules.toml exposes ${optionalEntries.length} optional modules for generated API composition`);

for (const entry of entries) {
  const pkg = inspectPackageManifest(entry);
  if (!pkg) {
    fail(`${entry.slug}: missing package-local rustok-module.toml at ${entry.modulePath}`);
    continue;
  }
  if (pkg.slugMatches) pass(`${entry.slug}: package manifest slug matches modules.toml`);
  else fail(`${entry.slug}: package manifest slug does not match modules.toml`);

  if (pkg.hasEntryType) pass(`${entry.slug}: package manifest declares crate entry_type`);
  else fail(`${entry.slug}: package manifest missing [crate].entry_type`);

  if ((pkg.hasGraphql || pkg.hasHttp) && !tableContains('docs/modules/registry.md', entry.slug)) {
    fail(`${entry.slug}: publishes API transport but is absent from central module registry`);
  } else if (pkg.hasGraphql || pkg.hasHttp) {
    pass(`${entry.slug}: API transport declaration is represented in central registry`);
  }
}

console.log('API surface contract verification');
for (const message of passes) console.log(`✓ ${message}`);
if (failures.length > 0) {
  for (const message of failures) console.error(`✗ ${message}`);
  console.error(`\n${failures.length} API surface contract violation(s)`);
  process.exit(1);
}
console.log(`\nAll ${passes.length} API surface contract checks passed`);
