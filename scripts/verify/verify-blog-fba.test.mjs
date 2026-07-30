#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  BLOG_FBA_SELF_TEST,
  BLOG_FBA_SELF_TEST_COMMAND,
  BLOG_FBA_SOURCE_GATES,
  BLOG_FBA_VERIFICATION_STEPS,
  collectBlogFbaVerificationChainFailures,
} from './blog-fba-verification-chain.mjs';

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function canonicalRegistry() {
  return {
    verification_chain: {
      package_script: 'verify:blog:fba',
      self_test: BLOG_FBA_SELF_TEST,
      steps: [...BLOG_FBA_VERIFICATION_STEPS],
      source_gates: clone(BLOG_FBA_SOURCE_GATES),
    },
  };
}

function canonicalPackageJson() {
  const scripts = {
    'verify:blog:fba': BLOG_FBA_VERIFICATION_STEPS.join(' && '),
    'test:verify:blog:fba': BLOG_FBA_SELF_TEST_COMMAND,
  };
  for (const gate of Object.values(BLOG_FBA_SOURCE_GATES)) {
    scripts[gate.package_script] = `node ${gate.verifier}`;
  }
  return { scripts };
}

function canonicalExistingPaths() {
  return new Set([
    BLOG_FBA_SELF_TEST,
    ...Object.values(BLOG_FBA_SOURCE_GATES).flatMap((gate) => [gate.verifier, gate.evidence]),
  ]);
}

function failures({ registry = canonicalRegistry(), packageJson = canonicalPackageJson(), existingPaths = canonicalExistingPaths() } = {}) {
  return collectBlogFbaVerificationChainFailures({
    registry,
    packageJson,
    existsSync: (filePath) => existingPaths.has(filePath),
  });
}

test('Blog FBA verification-chain policy accepts the canonical registry and package scripts', () => {
  assert.deepEqual(failures(), []);
});

test('Blog FBA verification-chain policy rejects removal of the storefront step', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.steps = registry.verification_chain.steps.filter(
    (step) => step !== 'npm run verify:blog:storefront-boundary',
  );
  assert.ok(failures({ registry }).includes('registry verification chain steps drift'));
});

test('Blog FBA verification-chain policy rejects package and registry order drift', () => {
  const packageJson = canonicalPackageJson();
  packageJson.scripts['verify:blog:fba'] = BLOG_FBA_VERIFICATION_STEPS
    .filter((step) => step !== 'npm run verify:blog:storefront-boundary')
    .join(' && ');
  assert.ok(failures({ packageJson }).includes('package verification chain steps drift'));
});

test('Blog FBA verification-chain policy rejects source-gate path drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.source_gates.storefront_boundary.evidence = 'wrong/storefront-evidence.json';
  assert.ok(failures({ registry }).includes('registry source gate storefront_boundary path drift'));
});

test('Blog FBA verification-chain policy rejects registry leaf-script drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.source_gates.storefront_boundary.package_script = 'verify:blog:wrong-storefront';
  assert.ok(
    failures({ registry }).includes('registry source gate storefront_boundary package script drift'),
  );
});

test('Blog FBA verification-chain policy rejects a repointed leaf verifier command', () => {
  const packageJson = canonicalPackageJson();
  packageJson.scripts['verify:blog:storefront-boundary'] = 'node scripts/verify/verify-blog-admin-boundary.mjs';
  assert.ok(failures({ packageJson }).includes('package source gate storefront_boundary command drift'));
});

test('Blog FBA verification-chain policy rejects a missing leaf verifier script', () => {
  const packageJson = canonicalPackageJson();
  delete packageJson.scripts['verify:blog:admin-boundary'];
  assert.ok(
    failures({ packageJson }).includes('package.json missing source gate script verify:blog:admin-boundary'),
  );
});

test('Blog FBA verification-chain policy rejects a missing registered source-gate file', () => {
  const existingPaths = canonicalExistingPaths();
  existingPaths.delete(BLOG_FBA_SOURCE_GATES.storefront_boundary.verifier);
  assert.ok(
    failures({ existingPaths }).includes(
      `registry source gate storefront_boundary missing ${BLOG_FBA_SOURCE_GATES.storefront_boundary.verifier}`,
    ),
  );
});

test('Blog FBA verification-chain policy rejects self-test path drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.self_test = 'scripts/verify/wrong-blog-fba.test.mjs';
  assert.ok(failures({ registry }).includes('verification chain self-test path drift'));
});

test('Blog FBA verification-chain policy rejects package self-test script drift', () => {
  const packageJson = canonicalPackageJson();
  packageJson.scripts['test:verify:blog:fba'] = 'node scripts/verify/wrong-blog-fba.test.mjs';
  assert.ok(failures({ packageJson }).includes('package Blog FBA self-test script drift'));
});

test('Blog FBA verification-chain policy rejects a missing self-test file', () => {
  const existingPaths = canonicalExistingPaths();
  existingPaths.delete(BLOG_FBA_SELF_TEST);
  assert.ok(
    failures({ existingPaths }).includes(`verification chain missing self-test ${BLOG_FBA_SELF_TEST}`),
  );
});
