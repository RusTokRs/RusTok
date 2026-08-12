#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST,
  BLOG_FBA_SELF_TEST,
  BLOG_FBA_SOURCE_GATES,
  BLOG_FBA_TEST_STEPS,
  BLOG_FBA_VERIFICATION_STEPS,
  collectBlogFbaVerificationChainFailures,
} from './blog-fba-verification-chain.mjs';

const commentsPortVerifier = 'scripts/verify/verify-blog-comments-port-boundary.mjs';
const commentsPortSelfTest = 'scripts/verify/verify-blog-comments-port-boundary.test.mjs';
const httpVerifierImport = "import './verify-blog-comments-http-port-injection.mjs';";
const httpSelfTestImport = "import './verify-blog-comments-http-port-injection.test.mjs';";
const graphqlVerifierImport = "import './verify-blog-comments-graphql-port-injection.mjs';";
const graphqlSelfTestImport = "import './verify-blog-comments-graphql-port-injection.test.mjs';";
const storefrontNativeVerifierImport =
  "import './verify-blog-comments-storefront-native-port-injection.mjs';";
const storefrontNativeSelfTestImport =
  "import './verify-blog-comments-storefront-native-port-injection.test.mjs';";
const adminNativeVerifierImport =
  "import './verify-blog-comments-admin-native-port-injection.mjs';";
const adminNativeSelfTestImport =
  "import './verify-blog-comments-admin-native-port-injection.test.mjs';";

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function canonicalRegistry() {
  return {
    verification_chain: {
      package_script: 'verify:blog:fba',
      self_test: BLOG_FBA_SELF_TEST,
      test_steps: [...BLOG_FBA_TEST_STEPS],
      consumer_runtime_self_test: clone(BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST),
      steps: [...BLOG_FBA_VERIFICATION_STEPS],
      source_gates: clone(BLOG_FBA_SOURCE_GATES),
    },
  };
}

function canonicalPackageJson() {
  const scripts = {
    'verify:blog:fba': BLOG_FBA_VERIFICATION_STEPS.join(' && '),
    'test:verify:blog:fba': BLOG_FBA_TEST_STEPS.join(' && '),
    [BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST.package_script]:
      `node ${BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST.self_test}`,
  };
  for (const gate of Object.values(BLOG_FBA_SOURCE_GATES)) {
    scripts[gate.package_script] = `node ${gate.verifier}`;
    scripts[gate.test_package_script] = `node ${gate.self_test}`;
  }
  return { scripts };
}

function canonicalExistingPaths() {
  return new Set([
    BLOG_FBA_SELF_TEST,
    BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST.self_test,
    ...Object.values(BLOG_FBA_SOURCE_GATES).flatMap((gate) => [
      gate.verifier,
      gate.self_test,
      gate.evidence,
      gate.unit_test,
      gate.postgres_test,
      gate.restart_test,
    ].filter(Boolean)),
  ]);
}

function failures({
  registry = canonicalRegistry(),
  packageJson = canonicalPackageJson(),
  existingPaths = canonicalExistingPaths(),
} = {}) {
  return collectBlogFbaVerificationChainFailures({
    registry,
    packageJson,
    existsSync: (filePath) => existingPaths.has(filePath),
  });
}

test('Blog FBA verification-chain policy accepts canonical verify and test chains', () => {
  assert.deepEqual(failures(), []);
});

test('Blog FBA policy retains HTTP composition verifier inside the registered Comments port gate', () => {
  const source = readFileSync(commentsPortVerifier, 'utf8');
  assert.ok(source.includes(httpVerifierImport));
});

test('Blog FBA policy retains HTTP composition focused fixture inside the registered Comments port self-test', () => {
  const source = readFileSync(commentsPortSelfTest, 'utf8');
  assert.ok(source.includes(httpSelfTestImport));
});

test('Blog FBA policy retains GraphQL composition verifier inside the registered Comments port gate', () => {
  const source = readFileSync(commentsPortVerifier, 'utf8');
  assert.ok(source.includes(graphqlVerifierImport));
});

test('Blog FBA policy retains GraphQL composition focused fixture inside the registered Comments port self-test', () => {
  const source = readFileSync(commentsPortSelfTest, 'utf8');
  assert.ok(source.includes(graphqlSelfTestImport));
});

test('Blog FBA policy retains storefront native composition verifier inside the registered Comments port gate', () => {
  const source = readFileSync(commentsPortVerifier, 'utf8');
  assert.ok(source.includes(storefrontNativeVerifierImport));
});

test('Blog FBA policy retains storefront native composition focused fixture inside the registered Comments port self-test', () => {
  const source = readFileSync(commentsPortSelfTest, 'utf8');
  assert.ok(source.includes(storefrontNativeSelfTestImport));
});

test('Blog FBA policy retains admin native composition verifier inside the registered Comments port gate', () => {
  const source = readFileSync(commentsPortVerifier, 'utf8');
  assert.ok(source.includes(adminNativeVerifierImport));
});

test('Blog FBA policy retains admin native composition focused fixture inside the registered Comments port self-test', () => {
  const source = readFileSync(commentsPortSelfTest, 'utf8');
  assert.ok(source.includes(adminNativeSelfTestImport));
});

test('Blog FBA verification-chain policy rejects removal of the storefront verify step', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.steps = registry.verification_chain.steps.filter(
    (step) => step !== 'npm run verify:blog:storefront-boundary',
  );
  assert.ok(failures({ registry }).includes('registry verification chain steps drift'));
});

test('Blog FBA verification-chain policy rejects removal of the duplicate-delivery verify step', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.steps = registry.verification_chain.steps.filter(
    (step) => step !== 'npm run verify:blog:comments-duplicate-delivery-race',
  );
  assert.ok(failures({ registry }).includes('registry verification chain steps drift'));
});

test('Blog FBA verification-chain policy rejects removal of the dispatcher duplicate-delivery verify step', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.steps = registry.verification_chain.steps.filter(
    (step) => step !== 'npm run verify:blog:comments-dispatcher-duplicate-delivery',
  );
  assert.ok(failures({ registry }).includes('registry verification chain steps drift'));
});

test('Blog FBA verification-chain policy rejects package verify-chain drift', () => {
  const packageJson = canonicalPackageJson();
  packageJson.scripts['verify:blog:fba'] = BLOG_FBA_VERIFICATION_STEPS
    .filter((step) => step !== 'npm run verify:blog:storefront-boundary')
    .join(' && ');
  assert.ok(failures({ packageJson }).includes('package verification chain steps drift'));
});

test('Blog FBA verification-chain policy rejects removal of a leaf self-test step', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.test_steps = registry.verification_chain.test_steps.filter(
    (step) => step !== 'npm run test:verify:blog:ai-richtext-boundary',
  );
  assert.ok(failures({ registry }).includes('registry test chain steps drift'));
});

test('Blog FBA verification-chain policy rejects package test-chain drift', () => {
  const packageJson = canonicalPackageJson();
  packageJson.scripts['test:verify:blog:fba'] = BLOG_FBA_TEST_STEPS
    .filter((step) => step !== 'npm run test:verify:blog:forum-ui-ownership')
    .join(' && ');
  assert.ok(failures({ packageJson }).includes('package Blog FBA test chain steps drift'));
});

test('Blog FBA verification-chain policy rejects source-gate path drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.source_gates.storefront_boundary.evidence = 'wrong/storefront-evidence.json';
  assert.ok(failures({ registry }).includes('registry source gate storefront_boundary path drift'));
});

test('Blog FBA verification-chain policy rejects projection unit-test path drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.source_gates.comments_event_projection.unit_test =
    'crates/rustok-blog/src/services/wrong_projection.rs';
  assert.ok(
    failures({ registry }).includes(
      'registry source gate comments_event_projection path drift',
    ),
  );
});

test('Blog FBA verification-chain policy rejects projection PostgreSQL-test path drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.source_gates.comments_event_projection.postgres_test =
    'crates/rustok-blog/tests/wrong_projection_postgres_test.rs';
  assert.ok(
    failures({ registry }).includes(
      'registry source gate comments_event_projection path drift',
    ),
  );
});

test('Blog FBA verification-chain policy rejects projection restart-test path drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.source_gates.comments_event_projection.restart_test =
    'crates/rustok-blog/tests/wrong_projection_restart_test.rs';
  assert.ok(
    failures({ registry }).includes(
      'registry source gate comments_event_projection path drift',
    ),
  );
});

test('Blog FBA verification-chain policy rejects duplicate-delivery PostgreSQL-test path drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.source_gates.comments_duplicate_delivery_race.postgres_test =
    'crates/rustok-blog/tests/wrong_duplicate_race_postgres_test.rs';
  assert.ok(
    failures({ registry }).includes(
      'registry source gate comments_duplicate_delivery_race path drift',
    ),
  );
});

test('Blog FBA verification-chain policy rejects dispatcher duplicate-delivery PostgreSQL-test path drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.source_gates.comments_dispatcher_duplicate_delivery.postgres_test =
    'crates/rustok-blog/tests/wrong_dispatcher_duplicate_postgres_test.rs';
  assert.ok(
    failures({ registry }).includes(
      'registry source gate comments_dispatcher_duplicate_delivery path drift',
    ),
  );
});

test('Blog FBA verification-chain policy rejects registry leaf-script drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.source_gates.storefront_boundary.package_script = 'verify:blog:wrong-storefront';
  assert.ok(
    failures({ registry }).includes('registry source gate storefront_boundary package script drift'),
  );
});

test('Blog FBA verification-chain policy rejects registry leaf-test-script drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.source_gates.forum_ui_ownership.test_package_script =
    'test:verify:blog:wrong-forum';
  assert.ok(
    failures({ registry }).includes(
      'registry source gate forum_ui_ownership test package script drift',
    ),
  );
});

test('Blog FBA verification-chain policy rejects a repointed leaf verifier command', () => {
  const packageJson = canonicalPackageJson();
  packageJson.scripts['verify:blog:storefront-boundary'] =
    'node scripts/verify/verify-blog-admin-boundary.mjs';
  assert.ok(failures({ packageJson }).includes('package source gate storefront_boundary command drift'));
});

test('Blog FBA verification-chain policy rejects a repointed leaf self-test command', () => {
  const packageJson = canonicalPackageJson();
  packageJson.scripts['test:verify:blog:forum-ui-ownership'] =
    'node scripts/verify/verify-blog-admin-boundary.test.mjs';
  assert.ok(
    failures({ packageJson }).includes('package source gate forum_ui_ownership test command drift'),
  );
});

test('Blog FBA verification-chain policy rejects a missing leaf verifier script', () => {
  const packageJson = canonicalPackageJson();
  delete packageJson.scripts['verify:blog:admin-boundary'];
  assert.ok(
    failures({ packageJson }).includes('package.json missing source gate script verify:blog:admin-boundary'),
  );
});

test('Blog FBA verification-chain policy rejects a missing duplicate-delivery leaf verifier script', () => {
  const packageJson = canonicalPackageJson();
  delete packageJson.scripts['verify:blog:comments-duplicate-delivery-race'];
  assert.ok(
    failures({ packageJson }).includes(
      'package.json missing source gate script verify:blog:comments-duplicate-delivery-race',
    ),
  );
});

test('Blog FBA verification-chain policy rejects a missing dispatcher duplicate-delivery leaf verifier script', () => {
  const packageJson = canonicalPackageJson();
  delete packageJson.scripts['verify:blog:comments-dispatcher-duplicate-delivery'];
  assert.ok(
    failures({ packageJson }).includes(
      'package.json missing source gate script verify:blog:comments-dispatcher-duplicate-delivery',
    ),
  );
});

test('Blog FBA verification-chain policy rejects a missing registered verifier file', () => {
  const existingPaths = canonicalExistingPaths();
  existingPaths.delete(BLOG_FBA_SOURCE_GATES.storefront_boundary.verifier);
  assert.ok(
    failures({ existingPaths }).includes(
      `registry source gate storefront_boundary missing ${BLOG_FBA_SOURCE_GATES.storefront_boundary.verifier}`,
    ),
  );
});

test('Blog FBA verification-chain policy rejects a missing leaf self-test file', () => {
  const existingPaths = canonicalExistingPaths();
  existingPaths.delete(BLOG_FBA_SOURCE_GATES.ai_richtext_boundary.self_test);
  assert.ok(
    failures({ existingPaths }).includes(
      `registry source gate ai_richtext_boundary missing ${BLOG_FBA_SOURCE_GATES.ai_richtext_boundary.self_test}`,
    ),
  );
});

test('Blog FBA verification-chain policy rejects a missing projection unit-test source', () => {
  const existingPaths = canonicalExistingPaths();
  existingPaths.delete(BLOG_FBA_SOURCE_GATES.comments_event_projection.unit_test);
  assert.ok(
    failures({ existingPaths }).includes(
      `registry source gate comments_event_projection missing ${BLOG_FBA_SOURCE_GATES.comments_event_projection.unit_test}`,
    ),
  );
});

test('Blog FBA verification-chain policy rejects a missing projection PostgreSQL target', () => {
  const existingPaths = canonicalExistingPaths();
  existingPaths.delete(BLOG_FBA_SOURCE_GATES.comments_event_projection.postgres_test);
  assert.ok(
    failures({ existingPaths }).includes(
      `registry source gate comments_event_projection missing ${BLOG_FBA_SOURCE_GATES.comments_event_projection.postgres_test}`,
    ),
  );
});

test('Blog FBA verification-chain policy rejects a missing duplicate-delivery PostgreSQL target', () => {
  const existingPaths = canonicalExistingPaths();
  existingPaths.delete(BLOG_FBA_SOURCE_GATES.comments_duplicate_delivery_race.postgres_test);
  assert.ok(
    failures({ existingPaths }).includes(
      `registry source gate comments_duplicate_delivery_race missing ${BLOG_FBA_SOURCE_GATES.comments_duplicate_delivery_race.postgres_test}`,
    ),
  );
});

test('Blog FBA verification-chain policy rejects a missing dispatcher duplicate-delivery PostgreSQL target', () => {
  const existingPaths = canonicalExistingPaths();
  existingPaths.delete(BLOG_FBA_SOURCE_GATES.comments_dispatcher_duplicate_delivery.postgres_test);
  assert.ok(
    failures({ existingPaths }).includes(
      `registry source gate comments_dispatcher_duplicate_delivery missing ${BLOG_FBA_SOURCE_GATES.comments_dispatcher_duplicate_delivery.postgres_test}`,
    ),
  );
});

test('Blog FBA verification-chain policy rejects a missing projection restart target', () => {
  const existingPaths = canonicalExistingPaths();
  existingPaths.delete(BLOG_FBA_SOURCE_GATES.comments_event_projection.restart_test);
  assert.ok(
    failures({ existingPaths }).includes(
      `registry source gate comments_event_projection missing ${BLOG_FBA_SOURCE_GATES.comments_event_projection.restart_test}`,
    ),
  );
});

test('Blog FBA verification-chain policy rejects aggregate self-test path drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.self_test = 'scripts/verify/wrong-blog-fba.test.mjs';
  assert.ok(failures({ registry }).includes('verification chain self-test path drift'));
});

test('Blog FBA verification-chain policy rejects consumer runtime self-test drift', () => {
  const packageJson = canonicalPackageJson();
  packageJson.scripts[BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST.package_script] =
    'node scripts/verify/wrong-consumer-runtime-order.test.mjs';
  assert.ok(failures({ packageJson }).includes('consumer runtime self-test command drift'));
});

test('Blog FBA verification-chain policy rejects a missing aggregate self-test file', () => {
  const existingPaths = canonicalExistingPaths();
  existingPaths.delete(BLOG_FBA_SELF_TEST);
  assert.ok(
    failures({ existingPaths }).includes(`verification chain missing self-test ${BLOG_FBA_SELF_TEST}`),
  );
});
