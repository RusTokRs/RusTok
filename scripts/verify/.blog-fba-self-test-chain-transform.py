from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one replacement, found {count}: {old[:160]!r}")
    target.write_text(text.replace(old, new, 1))


package_path = "package.json"
registry_path = "crates/rustok-blog/contracts/blog-fba-registry.json"
plan_path = "crates/rustok-blog/docs/implementation-plan.md"
fba_verifier_path = "scripts/verify/verify-blog-fba.mjs"
policy_path = "scripts/verify/blog-fba-verification-chain.mjs"
fba_test_path = "scripts/verify/verify-blog-fba.test.mjs"
forum_verifier_path = "scripts/verify/verify-blog-forum-ui-ownership.mjs"
forum_test_path = "scripts/verify/verify-blog-forum-ui-ownership.test.mjs"
backfill_verifier_path = "scripts/verify/verify-blog-richtext-offline-backfill.mjs"
backfill_test_path = "scripts/verify/verify-blog-richtext-offline-backfill.test.mjs"

replace_once(
    package_path,
    '    "verify:blog:fba": "node scripts/verify/verify-blog-fba.mjs && npm run verify:blog:admin-boundary && npm run verify:blog:storefront-boundary && npm run verify:blog:graphql-richtext-boundary && npm run verify:blog:richtext-offline-backfill && npm run verify:blog:forum-ui-ownership && node scripts/verify/verify-consumer-fba-runtime-order.mjs",\n    "test:verify:blog:fba": "node scripts/verify/verify-blog-fba.test.mjs",',
    '    "verify:blog:fba": "node scripts/verify/verify-blog-fba.mjs && npm run verify:blog:admin-boundary && npm run verify:blog:storefront-boundary && npm run verify:blog:graphql-richtext-boundary && npm run verify:blog:richtext-offline-backfill && npm run verify:blog:forum-ui-ownership && node scripts/verify/verify-consumer-fba-runtime-order.mjs",\n    "test:verify:blog:richtext-offline-backfill": "node scripts/verify/verify-blog-richtext-offline-backfill.test.mjs",\n    "test:verify:blog:forum-ui-ownership": "node scripts/verify/verify-blog-forum-ui-ownership.test.mjs",\n    "test:verify:blog:fba": "node scripts/verify/verify-blog-fba.test.mjs && npm run test:verify:blog:admin-boundary && npm run test:verify:blog:storefront-boundary && npm run test:verify:blog:graphql-richtext-boundary && npm run test:verify:blog:richtext-offline-backfill && npm run test:verify:blog:forum-ui-ownership && npm run test:verify:consumer:fba-runtime-order",',
)

replace_once(registry_path, '  "schema_version": 4,', '  "schema_version": 5,')
replace_once(
    registry_path,
    '    "self_test": "scripts/verify/verify-blog-fba.test.mjs",\n    "steps": [',
    '    "self_test": "scripts/verify/verify-blog-fba.test.mjs",\n    "test_steps": [\n      "node scripts/verify/verify-blog-fba.test.mjs",\n      "npm run test:verify:blog:admin-boundary",\n      "npm run test:verify:blog:storefront-boundary",\n      "npm run test:verify:blog:graphql-richtext-boundary",\n      "npm run test:verify:blog:richtext-offline-backfill",\n      "npm run test:verify:blog:forum-ui-ownership",\n      "npm run test:verify:consumer:fba-runtime-order"\n    ],\n    "consumer_runtime_self_test": {\n      "package_script": "test:verify:consumer:fba-runtime-order",\n      "self_test": "scripts/verify/verify-consumer-fba-runtime-order.test.mjs"\n    },\n    "steps": [',
)
for gate_name, package_script, verifier, evidence, test_package_script, self_test in [
    (
        "admin_boundary",
        "verify:blog:admin-boundary",
        "scripts/verify/verify-blog-admin-boundary.mjs",
        "crates/rustok-blog/contracts/evidence/blog-admin-richtext-boundary.json",
        "test:verify:blog:admin-boundary",
        "scripts/verify/verify-blog-admin-boundary.test.mjs",
    ),
    (
        "storefront_boundary",
        "verify:blog:storefront-boundary",
        "scripts/verify/verify-blog-storefront-boundary.mjs",
        "crates/rustok-blog/contracts/evidence/blog-storefront-richtext-view.json",
        "test:verify:blog:storefront-boundary",
        "scripts/verify/verify-blog-storefront-boundary.test.mjs",
    ),
    (
        "graphql_richtext_boundary",
        "verify:blog:graphql-richtext-boundary",
        "scripts/verify/verify-blog-graphql-richtext-boundary.mjs",
        "crates/rustok-blog/contracts/evidence/blog-graphql-richtext-boundary.json",
        "test:verify:blog:graphql-richtext-boundary",
        "scripts/verify/verify-blog-graphql-richtext-boundary.test.mjs",
    ),
    (
        "richtext_offline_backfill",
        "verify:blog:richtext-offline-backfill",
        "scripts/verify/verify-blog-richtext-offline-backfill.mjs",
        "crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json",
        "test:verify:blog:richtext-offline-backfill",
        "scripts/verify/verify-blog-richtext-offline-backfill.test.mjs",
    ),
    (
        "forum_ui_ownership",
        "verify:blog:forum-ui-ownership",
        "scripts/verify/verify-blog-forum-ui-ownership.mjs",
        "crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json",
        "test:verify:blog:forum-ui-ownership",
        "scripts/verify/verify-blog-forum-ui-ownership.test.mjs",
    ),
]:
    old = f'''      "{gate_name}": {{
        "package_script": "{package_script}",
        "verifier": "{verifier}",
        "evidence": "{evidence}"
      }}'''
    new = f'''      "{gate_name}": {{
        "package_script": "{package_script}",
        "test_package_script": "{test_package_script}",
        "verifier": "{verifier}",
        "self_test": "{self_test}",
        "evidence": "{evidence}"
      }}'''
    replace_once(registry_path, old, new)

Path(policy_path).write_text(r'''import fs from 'node:fs';

export const BLOG_FBA_VERIFICATION_STEPS = [
  'node scripts/verify/verify-blog-fba.mjs',
  'npm run verify:blog:admin-boundary',
  'npm run verify:blog:storefront-boundary',
  'npm run verify:blog:graphql-richtext-boundary',
  'npm run verify:blog:richtext-offline-backfill',
  'npm run verify:blog:forum-ui-ownership',
  'node scripts/verify/verify-consumer-fba-runtime-order.mjs',
];

export const BLOG_FBA_SOURCE_GATES = {
  admin_boundary: {
    package_script: 'verify:blog:admin-boundary',
    test_package_script: 'test:verify:blog:admin-boundary',
    verifier: 'scripts/verify/verify-blog-admin-boundary.mjs',
    self_test: 'scripts/verify/verify-blog-admin-boundary.test.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-admin-richtext-boundary.json',
  },
  storefront_boundary: {
    package_script: 'verify:blog:storefront-boundary',
    test_package_script: 'test:verify:blog:storefront-boundary',
    verifier: 'scripts/verify/verify-blog-storefront-boundary.mjs',
    self_test: 'scripts/verify/verify-blog-storefront-boundary.test.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-storefront-richtext-view.json',
  },
  graphql_richtext_boundary: {
    package_script: 'verify:blog:graphql-richtext-boundary',
    test_package_script: 'test:verify:blog:graphql-richtext-boundary',
    verifier: 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs',
    self_test: 'scripts/verify/verify-blog-graphql-richtext-boundary.test.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-graphql-richtext-boundary.json',
  },
  richtext_offline_backfill: {
    package_script: 'verify:blog:richtext-offline-backfill',
    test_package_script: 'test:verify:blog:richtext-offline-backfill',
    verifier: 'scripts/verify/verify-blog-richtext-offline-backfill.mjs',
    self_test: 'scripts/verify/verify-blog-richtext-offline-backfill.test.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json',
  },
  forum_ui_ownership: {
    package_script: 'verify:blog:forum-ui-ownership',
    test_package_script: 'test:verify:blog:forum-ui-ownership',
    verifier: 'scripts/verify/verify-blog-forum-ui-ownership.mjs',
    self_test: 'scripts/verify/verify-blog-forum-ui-ownership.test.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json',
  },
};

export const BLOG_FBA_SELF_TEST = 'scripts/verify/verify-blog-fba.test.mjs';
export const BLOG_FBA_SELF_TEST_COMMAND = `node ${BLOG_FBA_SELF_TEST}`;
export const BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST = {
  package_script: 'test:verify:consumer:fba-runtime-order',
  self_test: 'scripts/verify/verify-consumer-fba-runtime-order.test.mjs',
};
export const BLOG_FBA_TEST_STEPS = [
  BLOG_FBA_SELF_TEST_COMMAND,
  'npm run test:verify:blog:admin-boundary',
  'npm run test:verify:blog:storefront-boundary',
  'npm run test:verify:blog:graphql-richtext-boundary',
  'npm run test:verify:blog:richtext-offline-backfill',
  'npm run test:verify:blog:forum-ui-ownership',
  'npm run test:verify:consumer:fba-runtime-order',
];

function sameList(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function sameSet(actual, expected) {
  return [...actual].sort().join('|') === [...expected].sort().join('|');
}

export function collectBlogFbaVerificationChainFailures({
  registry,
  packageJson,
  existsSync = fs.existsSync,
} = {}) {
  const failures = [];
  const chain = registry?.verification_chain;

  if (chain?.package_script !== 'verify:blog:fba') {
    failures.push('verification chain package script drift');
  }
  if (chain?.self_test !== BLOG_FBA_SELF_TEST) {
    failures.push('verification chain self-test path drift');
  }
  if (!sameList(chain?.steps ?? [], BLOG_FBA_VERIFICATION_STEPS)) {
    failures.push('registry verification chain steps drift');
  }
  if (!sameList(chain?.test_steps ?? [], BLOG_FBA_TEST_STEPS)) {
    failures.push('registry test chain steps drift');
  }

  const packageScript = packageJson?.scripts?.['verify:blog:fba'];
  if (typeof packageScript !== 'string') {
    failures.push('package.json missing verify:blog:fba script');
  } else if (!sameList(packageScript.split(' && '), BLOG_FBA_VERIFICATION_STEPS)) {
    failures.push('package verification chain steps drift');
  }

  const packageTestScript = packageJson?.scripts?.['test:verify:blog:fba'];
  if (typeof packageTestScript !== 'string') {
    failures.push('package.json missing test:verify:blog:fba script');
  } else if (!sameList(packageTestScript.split(' && '), BLOG_FBA_TEST_STEPS)) {
    failures.push('package Blog FBA test chain steps drift');
  }

  const sourceGates = chain?.source_gates ?? {};
  if (!sameSet(Object.keys(sourceGates), Object.keys(BLOG_FBA_SOURCE_GATES))) {
    failures.push('registry source gate names drift');
  }

  for (const [gateName, expectedGate] of Object.entries(BLOG_FBA_SOURCE_GATES)) {
    const gate = sourceGates[gateName];
    if (gate?.package_script !== expectedGate.package_script) {
      failures.push(`registry source gate ${gateName} package script drift`);
    }
    if (gate?.test_package_script !== expectedGate.test_package_script) {
      failures.push(`registry source gate ${gateName} test package script drift`);
    }
    if (
      gate?.verifier !== expectedGate.verifier ||
      gate?.self_test !== expectedGate.self_test ||
      gate?.evidence !== expectedGate.evidence
    ) {
      failures.push(`registry source gate ${gateName} path drift`);
    }

    const leafCommand = packageJson?.scripts?.[expectedGate.package_script];
    const expectedLeafCommand = `node ${expectedGate.verifier}`;
    if (typeof leafCommand !== 'string') {
      failures.push(`package.json missing source gate script ${expectedGate.package_script}`);
    } else if (leafCommand !== expectedLeafCommand) {
      failures.push(`package source gate ${gateName} command drift`);
    }

    const leafTestCommand = packageJson?.scripts?.[expectedGate.test_package_script];
    const expectedLeafTestCommand = `node ${expectedGate.self_test}`;
    if (typeof leafTestCommand !== 'string') {
      failures.push(`package.json missing source gate test script ${expectedGate.test_package_script}`);
    } else if (leafTestCommand !== expectedLeafTestCommand) {
      failures.push(`package source gate ${gateName} test command drift`);
    }

    for (const filePath of [expectedGate.verifier, expectedGate.self_test, expectedGate.evidence]) {
      if (!existsSync(filePath)) {
        failures.push(`registry source gate ${gateName} missing ${filePath}`);
      }
    }
  }

  const runtimeSelfTest = chain?.consumer_runtime_self_test;
  if (
    runtimeSelfTest?.package_script !== BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST.package_script ||
    runtimeSelfTest?.self_test !== BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST.self_test
  ) {
    failures.push('consumer runtime self-test registry drift');
  }
  const runtimeSelfTestCommand =
    packageJson?.scripts?.[BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST.package_script];
  const expectedRuntimeSelfTestCommand =
    `node ${BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST.self_test}`;
  if (runtimeSelfTestCommand !== expectedRuntimeSelfTestCommand) {
    failures.push('consumer runtime self-test command drift');
  }
  if (!existsSync(BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST.self_test)) {
    failures.push(
      `verification chain missing consumer runtime self-test ${BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST.self_test}`,
    );
  }

  if (!existsSync(BLOG_FBA_SELF_TEST)) {
    failures.push(`verification chain missing self-test ${BLOG_FBA_SELF_TEST}`);
  }

  return failures;
}
''')

Path(fba_test_path).write_text(r'''#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  BLOG_FBA_CONSUMER_RUNTIME_SELF_TEST,
  BLOG_FBA_SELF_TEST,
  BLOG_FBA_SOURCE_GATES,
  BLOG_FBA_TEST_STEPS,
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
    ]),
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

test('Blog FBA verification-chain policy rejects removal of the storefront verify step', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.steps = registry.verification_chain.steps.filter(
    (step) => step !== 'npm run verify:blog:storefront-boundary',
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
    (step) => step !== 'npm run test:verify:blog:richtext-offline-backfill',
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

test('Blog FBA verification-chain policy rejects a missing leaf self-test file', () => {
  const existingPaths = canonicalExistingPaths();
  existingPaths.delete(BLOG_FBA_SOURCE_GATES.richtext_offline_backfill.self_test);
  assert.ok(
    failures({ existingPaths }).includes(
      `registry source gate richtext_offline_backfill missing ${BLOG_FBA_SOURCE_GATES.richtext_offline_backfill.self_test}`,
    ),
  );
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
''')

Path(forum_verifier_path).write_text(r'''#!/usr/bin/env node

import fs from 'node:fs';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function fail(message) {
  console.error(`[verify-blog-forum-ui-ownership] ${message}`);
  process.exit(1);
}

function requireFile(path) {
  if (!fs.existsSync(path)) fail(`${path} is missing`);
  return read(path);
}

function requireAbsent(path) {
  if (fs.existsSync(path)) fail(`${path} must be removed from the Blog package`);
}

function hasAll(text, markers, label) {
  for (const marker of markers) {
    if (!text.includes(marker)) fail(`${label} missing ${marker}`);
  }
}

function hasNone(text, markers, label) {
  for (const marker of markers) {
    if (text.includes(marker)) fail(`${label} contains forbidden ${marker}`);
  }
}

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json';
const evidence = JSON.parse(requireFile(evidencePath));
if (
  evidence.schema_version !== 1 ||
  evidence.module !== 'blog' ||
  evidence.surface !== 'next_admin_forum_ui_ownership' ||
  evidence.status !== 'source_verified_no_compile' ||
  evidence.compile_policy !== 'not_run_by_request'
) {
  fail('evidence identity/status drift');
}

const blogIndex = requireFile('apps/next-admin/packages/blog/src/index.ts');
const blogNav = requireFile('apps/next-admin/packages/blog/src/nav.ts');
const blogPostForm = requireFile(
  'apps/next-admin/packages/blog/src/components/post-form.tsx'
);
const forumIndex = requireFile('apps/next-admin/packages/forum/src/index.ts');
const forumNav = requireFile('apps/next-admin/packages/forum/src/nav.ts');
const forumApi = requireFile('apps/next-admin/packages/forum/src/api/forum.ts');
const forumEditor = requireFile(
  'apps/next-admin/packages/forum/src/components/forum-reply-editor.tsx'
);
const forumLegacyAdapter = requireFile(
  'apps/next-admin/packages/forum/src/components/rt-json-format.ts'
);
const sharedEditor = requireFile(
  'apps/next-admin/src/shared/ui/rich-text-editor.tsx'
);
const modulesIndex = requireFile('apps/next-admin/src/modules/index.ts');
const forumPage = requireFile(
  'apps/next-admin/src/app/dashboard/forum/reply/page.tsx'
);

for (const path of [
  'apps/next-admin/packages/blog/src/api/forum.ts',
  'apps/next-admin/packages/blog/src/components/forum-reply-editor.tsx',
  'apps/next-admin/packages/blog/src/components/rt-json-format.ts',
  'apps/next-admin/packages/blog/src/components/rich-text-editor.tsx'
]) {
  requireAbsent(path);
}

hasAll(blogIndex, ["id: 'blog'", "export { blogNavItems } from './nav'"], 'Blog index');
hasNone(
  blogIndex,
  ["id: 'forum'", 'forumNavItems', 'ForumReplyEditor', "./api/forum", 'RichTextEditor'],
  'Blog index'
);
hasNone(blogNav, ['forumNavItems', "title: 'Forum'", '/dashboard/forum'], 'Blog navigation');
hasAll(
  blogPostForm,
  ["@/shared/ui/rich-text-editor", "profile='article'"],
  'Blog post form'
);
hasNone(blogPostForm, ["./rich-text-editor"], 'Blog post form');

hasAll(
  forumIndex,
  ["id: 'forum'", 'forumNavItems', 'ForumReplyEditor', "export * from './api/forum'"],
  'Forum index'
);
hasAll(forumNav, ["title: 'Forum'", '/dashboard/forum/reply'], 'Forum navigation');
hasAll(
  forumApi,
  ['export interface GqlOpts', 'listForumTopics', 'createForumReply'],
  'Forum GraphQL adapter'
);
hasAll(
  forumEditor,
  [
    "@/shared/ui/rich-text-editor",
    "profile='discussion'",
    "from '../api/forum'",
    "from './rt-json-format'"
  ],
  'Forum reply editor'
);
hasNone(
  forumEditor,
  ['packages/blog', "../api/posts", "./rich-text-editor"],
  'Forum reply editor'
);
hasAll(
  forumLegacyAdapter,
  ['normalizeRtJsonPayload', 'stringifyRtDoc', "version: 'rt_json_v1'"],
  'Forum legacy adapter'
);
hasAll(
  sharedEditor,
  [
    "from '@rustok/richtext/react'",
    'profile: RichTextProfileId;',
    "frameUrl='/richtext/frame'"
  ],
  'Shared richtext adapter'
);
hasAll(modulesIndex, ["import '../../packages/blog/src';", "import '../../packages/forum/src';"], 'Host module registration');
hasAll(forumPage, ["../../../../../packages/forum/src", 'ForumReplyEditor', 'listForumTopics'], 'Forum route');
hasNone(forumPage, ['packages/blog/src'], 'Forum route');

if (
  evidence.owner_package !== 'apps/next-admin/packages/forum/src' ||
  evidence.former_owner_package !== 'apps/next-admin/packages/blog/src' ||
  evidence.shared_richtext_adapter !==
    'apps/next-admin/src/shared/ui/rich-text-editor.tsx' ||
  evidence.verifier !== 'scripts/verify/verify-blog-forum-ui-ownership.mjs'
) {
  fail('evidence path drift');
}

const packageJson = JSON.parse(requireFile('package.json'));
if (
  packageJson.scripts?.['verify:blog:forum-ui-ownership'] !==
  'node scripts/verify/verify-blog-forum-ui-ownership.mjs'
) {
  fail('package verifier command drift');
}
if (
  packageJson.scripts?.['test:verify:blog:forum-ui-ownership'] !==
  'node scripts/verify/verify-blog-forum-ui-ownership.test.mjs'
) {
  fail('package self-test command drift');
}
if (!packageJson.scripts?.['verify:blog:fba']?.includes('verify:blog:forum-ui-ownership')) {
  fail('Blog FBA aggregate does not include Forum ownership verifier');
}
if (!packageJson.scripts?.['test:verify:blog:fba']?.includes('test:verify:blog:forum-ui-ownership')) {
  fail('Blog FBA self-test aggregate does not include Forum ownership fixture');
}
requireFile('scripts/verify/verify-blog-forum-ui-ownership.test.mjs');

console.log(
  '[verify-blog-forum-ui-ownership] Forum Next admin navigation, API, reply editor, and legacy adapter are Forum-owned; Blog uses only the shared richtext lifecycle adapter'
);
''')

Path(forum_test_path).write_text(r'''#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const scriptPath = path.resolve('scripts/verify/verify-blog-forum-ui-ownership.mjs');

function writeFixtureFile(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-forum-ui-ownership-'));
  writeFixtureFile(root, 'apps/next-admin/packages/blog/src/index.ts',
    options.blogOwnsForum
      ? "id: 'blog'\nid: 'forum'\nforumNavItems\nForumReplyEditor\nexport { blogNavItems } from './nav'"
      : "id: 'blog'\nexport { blogNavItems } from './nav'");
  writeFixtureFile(root, 'apps/next-admin/packages/blog/src/nav.ts', 'export const blogNavItems = [];');
  writeFixtureFile(root, 'apps/next-admin/packages/blog/src/components/post-form.tsx',
    "@/shared/ui/rich-text-editor\nprofile='article'");
  writeFixtureFile(root, 'apps/next-admin/packages/forum/src/index.ts',
    "id: 'forum'\nforumNavItems\nForumReplyEditor\nexport * from './api/forum'");
  writeFixtureFile(root, 'apps/next-admin/packages/forum/src/nav.ts',
    "title: 'Forum'\n/dashboard/forum/reply");
  writeFixtureFile(root, 'apps/next-admin/packages/forum/src/api/forum.ts',
    'export interface GqlOpts\nlistForumTopics\ncreateForumReply');
  writeFixtureFile(root, 'apps/next-admin/packages/forum/src/components/forum-reply-editor.tsx',
    `@/shared/ui/rich-text-editor
profile='${options.articleProfile ? 'article' : 'discussion'}'
from '../api/forum'
from './rt-json-format'`);
  writeFixtureFile(root, 'apps/next-admin/packages/forum/src/components/rt-json-format.ts',
    "normalizeRtJsonPayload\nstringifyRtDoc\nversion: 'rt_json_v1'");
  writeFixtureFile(root, 'apps/next-admin/src/shared/ui/rich-text-editor.tsx',
    "from '@rustok/richtext/react'\nprofile: RichTextProfileId;\nframeUrl='/richtext/frame'");
  writeFixtureFile(root, 'apps/next-admin/src/modules/index.ts',
    "import '../../packages/blog/src';\nimport '../../packages/forum/src';");
  writeFixtureFile(root, 'apps/next-admin/src/app/dashboard/forum/reply/page.tsx',
    options.blogRoute
      ? "../../../../../packages/forum/src\n../../../../../packages/blog/src\nForumReplyEditor\nlistForumTopics"
      : "../../../../../packages/forum/src\nForumReplyEditor\nlistForumTopics");
  if (options.blogOwnsForum) {
    writeFixtureFile(root, 'apps/next-admin/packages/blog/src/api/forum.ts', 'legacy owner');
  }
  writeFixtureFile(root, 'crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json',
    JSON.stringify({
      schema_version: 1,
      module: 'blog',
      surface: 'next_admin_forum_ui_ownership',
      status: 'source_verified_no_compile',
      compile_policy: 'not_run_by_request',
      owner_package: options.evidenceDrift
        ? 'apps/next-admin/packages/blog/src'
        : 'apps/next-admin/packages/forum/src',
      former_owner_package: 'apps/next-admin/packages/blog/src',
      shared_richtext_adapter: 'apps/next-admin/src/shared/ui/rich-text-editor.tsx',
      verifier: 'scripts/verify/verify-blog-forum-ui-ownership.mjs',
    }));
  writeFixtureFile(root, 'scripts/verify/verify-blog-forum-ui-ownership.test.mjs', 'fixture marker');
  writeFixtureFile(root, 'package.json', JSON.stringify({
    scripts: {
      'verify:blog:forum-ui-ownership': 'node scripts/verify/verify-blog-forum-ui-ownership.mjs',
      'test:verify:blog:forum-ui-ownership': 'node scripts/verify/verify-blog-forum-ui-ownership.test.mjs',
      'verify:blog:fba': 'node scripts/verify/verify-blog-fba.mjs && npm run verify:blog:forum-ui-ownership',
      'test:verify:blog:fba': options.omitTestAggregate
        ? 'node scripts/verify/verify-blog-fba.test.mjs'
        : 'node scripts/verify/verify-blog-fba.test.mjs && npm run test:verify:blog:forum-ui-ownership',
    },
  }));
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [scriptPath], { cwd: root, encoding: 'utf8' });
}

test('Blog Forum UI ownership verifier accepts the canonical owner split', () => {
  const result = run(fixture());
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Forum Next admin navigation/);
});

test('Blog Forum UI ownership verifier rejects Forum files returning to Blog', () => {
  const result = run(fixture({ blogOwnsForum: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /must be removed from the Blog package|Blog index contains forbidden/);
});

test('Blog Forum UI ownership verifier rejects the Article profile in Forum editor', () => {
  const result = run(fixture({ articleProfile: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Forum reply editor missing profile='discussion'/);
});

test('Blog Forum UI ownership verifier rejects a Forum route importing Blog', () => {
  const result = run(fixture({ blogRoute: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Forum route contains forbidden packages\/blog\/src/);
});

test('Blog Forum UI ownership verifier rejects evidence ownership drift', () => {
  const result = run(fixture({ evidenceDrift: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /evidence path drift/);
});

test('Blog Forum UI ownership verifier rejects missing aggregate self-test wiring', () => {
  const result = run(fixture({ omitTestAggregate: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /self-test aggregate does not include Forum ownership fixture/);
});
''')

Path(backfill_verifier_path).write_text(r'''import fs from "node:fs";

function read(path) { return fs.readFileSync(path, "utf8"); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-blog-richtext-offline-backfill] ${message}`); process.exit(1); }
function hasAll(text, markers, label) {
  for (const marker of markers) if (!text.includes(marker)) fail(`${label} missing ${marker}`);
}
function hasNone(text, markers, label) {
  for (const marker of markers) if (text.includes(marker)) fail(`${label} contains forbidden ${marker}`);
}

const sourcePath = "crates/rustok-blog/src/bin/blog_article_richtext_backfill.rs";
const evidencePath = "crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json";
const inventoryPath = "crates/rustok-blog/contracts/evidence/blog-richtext-cutover-inventory.json";
const planPath = "crates/rustok-blog/docs/implementation-plan.md";
const inventoryDocPath = "crates/rustok-blog/docs/richtext-cutover-inventory.md";
const selfTestPath = "scripts/verify/verify-blog-richtext-offline-backfill.test.mjs";

const source = read(sourcePath);
const evidence = json(evidencePath);
const inventory = json(inventoryPath);
const plan = read(planPath);
const inventoryDoc = read(inventoryDocPath);
const packageJson = json("package.json");

if (evidence.schema_version !== 1 || evidence.module !== "blog" || evidence.surface !== "article_richtext_offline_backfill") {
  fail("evidence identity drift");
}
if (evidence.status !== "executable_no_run" || evidence.compile_policy !== "not_run_by_request") {
  fail("evidence status drift");
}
if (evidence.runner !== "cargo run -p rustok-blog --bin blog_article_richtext_backfill --") {
  fail("runner drift");
}
if (evidence.source !== sourcePath || evidence.verifier !== "scripts/verify/verify-blog-richtext-offline-backfill.mjs") {
  fail("source/verifier path drift");
}
if (evidence.safety?.default_mode !== "dry_run"
  || evidence.safety?.apply_flag !== "--apply"
  || evidence.safety?.markdown_plain_text_flag !== "--allow-markdown-plain-text"
  || evidence.safety?.preflight_before_apply !== true
  || evidence.safety?.optimistic_updates !== true
  || evidence.safety?.orphan_rows_fail_closed !== true
  || evidence.safety?.stable_cursor !== "updated_at_id"
  || evidence.safety?.optimistic_updated_at_predicate !== true
  || evidence.safety?.checkpoint_mutation !== false) {
  fail("safety contract drift");
}

hasAll(source, [
  "const TARGET_FORMAT: &str = \"richtext\";",
  "async fn preflight_pass(",
  "async fn apply_pass(",
  "async fn optimistic_update(",
  "LEFT JOIN blog_posts",
  "struct Cursor",
  "updated_at = $6",
  "repair the owner relation before backfill",
  "--apply",
  "--allow-markdown-plain-text",
  "article_document_from_plain_text",
  "normalize_article",
  "canonical_article_body",
  "full successful preflight",
  "optimistic update conflict",
  "post-apply verification failed",
  "ReportRecord",
  "body_format = $2",
  "body_format = ?",
], "backfill source");
hasNone(source, [
  "rustok_content::entities",
  "validate_and_sanitize_rt_json",
  "persist_checkpoint",
  "checkpoint_file",
  "CONTENT_FORMAT_MARKDOWN",
], "backfill source");

const check = inventory.checks?.find((entry) => entry.name === "offline_backfill");
if (!check || check.status !== "executable_no_run" || check.path !== sourcePath) {
  fail("inventory offline_backfill check drift");
}
if (!inventory.completion_conditions?.includes("legacy_rows_have_owner_specific_dry_run_backfill")) {
  fail("inventory completion condition missing");
}

hasAll(plan, [sourcePath, evidencePath, "--allow-markdown-plain-text", "offline backfill"], "implementation plan");
hasAll(inventoryDoc, [sourcePath, evidencePath, "Dry-run is the default", "optimistic"], "inventory documentation");

if (packageJson.scripts?.["verify:blog:richtext-offline-backfill"] !== "node scripts/verify/verify-blog-richtext-offline-backfill.mjs") {
  fail("package verifier command drift");
}
if (packageJson.scripts?.["test:verify:blog:richtext-offline-backfill"] !== `node ${selfTestPath}`) {
  fail("package self-test command drift");
}
if (!packageJson.scripts?.["verify:blog:fba"]?.includes("verify:blog:richtext-offline-backfill")) {
  fail("Blog FBA aggregate does not include offline backfill verifier");
}
if (!packageJson.scripts?.["test:verify:blog:fba"]?.includes("test:verify:blog:richtext-offline-backfill")) {
  fail("Blog FBA self-test aggregate does not include offline backfill fixture");
}
if (!fs.existsSync(selfTestPath)) {
  fail(`self-test file missing ${selfTestPath}`);
}

console.log("[verify-blog-richtext-offline-backfill] owner-specific dry-run/apply safety contract is consistent");
''')

Path(backfill_test_path).write_text(r'''#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const scriptPath = path.resolve('scripts/verify/verify-blog-richtext-offline-backfill.mjs');
const sourcePath = 'crates/rustok-blog/src/bin/blog_article_richtext_backfill.rs';
const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json';

function writeFixtureFile(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function sourceText(options = {}) {
  return `const TARGET_FORMAT: &str = "richtext";
async fn preflight_pass(
async fn apply_pass(
async fn optimistic_update(
LEFT JOIN blog_posts
struct Cursor
updated_at = $6
repair the owner relation before backfill
--apply
--allow-markdown-plain-text
article_document_from_plain_text
normalize_article
canonical_article_body
full successful preflight
${options.missingOptimistic ? '' : 'optimistic update conflict'}
post-apply verification failed
ReportRecord
body_format = $2
body_format = ?
${options.checkpoint ? 'persist_checkpoint' : ''}`;
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-richtext-backfill-'));
  writeFixtureFile(root, sourcePath, sourceText(options));
  writeFixtureFile(root, evidencePath, JSON.stringify({
    schema_version: 1,
    module: 'blog',
    surface: 'article_richtext_offline_backfill',
    status: 'executable_no_run',
    compile_policy: 'not_run_by_request',
    runner: 'cargo run -p rustok-blog --bin blog_article_richtext_backfill --',
    source: sourcePath,
    verifier: 'scripts/verify/verify-blog-richtext-offline-backfill.mjs',
    safety: {
      default_mode: options.unsafeDefault ? 'apply' : 'dry_run',
      apply_flag: '--apply',
      markdown_plain_text_flag: '--allow-markdown-plain-text',
      preflight_before_apply: true,
      optimistic_updates: true,
      orphan_rows_fail_closed: true,
      stable_cursor: 'updated_at_id',
      optimistic_updated_at_predicate: true,
      checkpoint_mutation: false,
    },
  }));
  writeFixtureFile(root, 'crates/rustok-blog/contracts/evidence/blog-richtext-cutover-inventory.json',
    JSON.stringify({
      checks: [{ name: 'offline_backfill', status: 'executable_no_run', path: sourcePath }],
      completion_conditions: ['legacy_rows_have_owner_specific_dry_run_backfill'],
    }));
  writeFixtureFile(root, 'crates/rustok-blog/docs/implementation-plan.md',
    `${sourcePath} ${evidencePath} --allow-markdown-plain-text offline backfill`);
  writeFixtureFile(root, 'crates/rustok-blog/docs/richtext-cutover-inventory.md',
    `${sourcePath} ${evidencePath} Dry-run is the default optimistic`);
  writeFixtureFile(root, 'scripts/verify/verify-blog-richtext-offline-backfill.test.mjs', 'fixture marker');
  writeFixtureFile(root, 'package.json', JSON.stringify({
    scripts: {
      'verify:blog:richtext-offline-backfill': 'node scripts/verify/verify-blog-richtext-offline-backfill.mjs',
      'test:verify:blog:richtext-offline-backfill': 'node scripts/verify/verify-blog-richtext-offline-backfill.test.mjs',
      'verify:blog:fba': 'node scripts/verify/verify-blog-fba.mjs && npm run verify:blog:richtext-offline-backfill',
      'test:verify:blog:fba': options.omitTestAggregate
        ? 'node scripts/verify/verify-blog-fba.test.mjs'
        : 'node scripts/verify/verify-blog-fba.test.mjs && npm run test:verify:blog:richtext-offline-backfill',
    },
  }));
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [scriptPath], { cwd: root, encoding: 'utf8' });
}

test('Blog richtext offline backfill verifier accepts the canonical dry-run contract', () => {
  const result = run(fixture());
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /dry-run\/apply safety contract is consistent/);
});

test('Blog richtext offline backfill verifier rejects apply-by-default evidence', () => {
  const result = run(fixture({ unsafeDefault: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /safety contract drift/);
});

test('Blog richtext offline backfill verifier rejects a missing optimistic conflict guard', () => {
  const result = run(fixture({ missingOptimistic: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /backfill source missing optimistic update conflict/);
});

test('Blog richtext offline backfill verifier rejects checkpoint mutation', () => {
  const result = run(fixture({ checkpoint: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /backfill source contains forbidden persist_checkpoint/);
});

test('Blog richtext offline backfill verifier rejects missing aggregate self-test wiring', () => {
  const result = run(fixture({ omitTestAggregate: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /self-test aggregate does not include offline backfill fixture/);
});
''')

replace_once(fba_verifier_path, "if (registry.schema_version !== 4)", "if (registry.schema_version !== 5)")

replace_once(
    plan_path,
    "Every registered leaf gate also binds its npm script to the exact verifier command;\nrenaming, removing, or repointing a `verify:blog:*` script now fails the aggregate\npolicy instead of leaving a no-op behind the correct aggregate step name.\n",
    "Every registered leaf gate also binds its npm script to the exact verifier command;\nrenaming, removing, or repointing a `verify:blog:*` script now fails the aggregate\npolicy instead of leaving a no-op behind the correct aggregate step name. The test\nside is equally locked: aggregate, admin, storefront, GraphQL richtext, offline\nbackfill, Forum ownership, and consumer runtime-order self-tests execute in one\nregistry-owned order. Offline backfill and Forum ownership now retain focused\ntemporary-repository negative fixtures instead of source assertions without proof.\n",
)
replace_once(
    plan_path,
    "- Blog FBA source-gate chain: `source_verified_no_compile`; registry schema v4\n  locks the exact package order, leaf npm-script-to-verifier commands, source-gate\n  paths, and aggregate self-test binding for admin, storefront, GraphQL richtext,\n  offline backfill, Forum ownership, and consumer runtime-order gates.",
    "- Blog FBA source-gate chain: `source_verified_no_compile`; registry schema v5\n  locks exact verify/test order, leaf npm-script-to-verifier and self-test commands,\n  source-gate paths, and aggregate/consumer self-test bindings for admin, storefront,\n  GraphQL richtext, offline backfill, Forum ownership, and runtime-order gates.",
)
replace_once(
    plan_path,
    "30. Bound every registered Blog FBA leaf npm script to its exact verifier command\n    and extended the aggregate policy fixture to reject missing, repointed, or\n    registry-renamed source-gate scripts.\n",
    "30. Bound every registered Blog FBA leaf npm script to its exact verifier command\n    and extended the aggregate policy fixture to reject missing, repointed, or\n    registry-renamed source-gate scripts.\n31. Locked the complete Blog FBA self-test chain in registry schema v5 and added\n    focused negative fixtures for offline-backfill safety and Forum Next admin\n    ownership, including exact leaf-test and consumer-runtime bindings.\n",
)
