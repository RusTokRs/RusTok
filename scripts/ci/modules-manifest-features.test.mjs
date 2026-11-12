import assert from 'node:assert/strict';
import { test } from 'node:test';

// The module exports are functions; the CLI body runs only when the file is the entry point, so
// importing it here evaluates no check.
import { compare, manifestSlugs, requiredFeatures } from './modules-manifest-features.mjs';

const manifest = (modules, extra = '') => `
schema = 2
[build]
profile = "release"

[modules]
${modules}
${extra}
`;

const server = (defaults) => `
[features]
default = [
${defaults.map((feature) => `    "${feature}",`).join('\n')}
]
mod-party = ["dep:rustok-party"]
mod-billing = ["dep:rustok-billing"]
mod-ai = ["dep:rustok-ai"]
`;

test('a declared module with an entry type needs its feature in the default build', () => {
  const result = compare(
    manifest('party = { crate = "rustok-party", source = "path", path = "crates/modules/rustok-party" }'),
    server(['redis-cache']),
  );
  assert.deepEqual(result.missingFromDefault, ['mod-party']);
  assert.deepEqual(result.absentFromManifest, []);
});

test('a required module is compiled unconditionally and needs no feature', () => {
  const result = compare(
    manifest('cache = { crate = "rustok-cache", source = "path", required = true }'),
    server(['redis-cache']),
  );
  assert.deepEqual(result.missingFromDefault, []);
  assert.equal(result.required.size, 0);
  // It is still a manifest slug, so a feature for it would not read as unexpected.
  assert.ok(manifestSlugs(manifest('cache = { crate = "rustok-cache", source = "path", required = true }')).has('cache'));
});

test('an extension is not required even though the default build compiles it', () => {
  // `ai` declares `runtime = "extension"` and has no `[crate] entry_type`, so the registry
  // contract does not require it; `mod-ai` in the default list is consistent, not a mismatch.
  const result = compare(
    manifest('ai = { crate = "rustok-ai", source = "path", runtime = "extension" }'),
    server(['mod-ai']),
  );
  assert.deepEqual(result.missingFromDefault, []);
  assert.deepEqual(result.absentFromManifest, []);
});

test('a compiled feature for a module the manifest never declares is unexpected', () => {
  const result = compare(manifest(''), server(['mod-billing']));
  assert.deepEqual(result.absentFromManifest, ['billing']);
  assert.equal(result.missingFromDefault.length, 0);
});

test('a module with no package entry type contributes no requirement', () => {
  // A slug that resolves to no package on disk has no `entry_type`, so it is compiled at most.
  const result = requiredFeatures(
    manifest('ghost_module = { crate = "ghost", source = "path", path = "crates/modules/ghost" }'),
    new Set(['mod-ghost_module']),
  );
  assert.equal(result.missingFeature.length, 0);
  assert.equal(result.required.size, 0);
});

test('the inline-table parser ignores comments and section headers', () => {
  const slugs = manifestSlugs(`
[modules]
# a domain module
party = { crate = "rustok-party", source = "path", depends_on = ["outbox", "tenant"] }

[settings]
default_enabled = ["party"]
`);
  assert.deepEqual([...slugs], ['party']);
});
