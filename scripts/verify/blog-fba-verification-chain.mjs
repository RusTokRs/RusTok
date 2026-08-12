import fs from 'node:fs';

export const BLOG_FBA_VERIFICATION_STEPS = [
  'node scripts/verify/verify-blog-fba.mjs',
  'npm run verify:blog:admin-boundary',
  'npm run verify:blog:storefront-boundary',
  'npm run verify:blog:comments-port-boundary',
  'npm run verify:blog:comments-event-projection',
  'npm run verify:blog:comments-duplicate-delivery-race',
  'npm run verify:blog:comments-dispatcher-duplicate-delivery',
  'npm run verify:blog:category-search-reindex',
  'npm run verify:blog:graphql-rate-limit',
  'npm run verify:blog:graphql-richtext-boundary',
  'npm run verify:blog:ai-richtext-boundary',
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
  comments_port_boundary: {
    package_script: 'verify:blog:comments-port-boundary',
    test_package_script: 'test:verify:blog:comments-port-boundary',
    verifier: 'scripts/verify/verify-blog-comments-port-boundary.mjs',
    self_test: 'scripts/verify/verify-blog-comments-port-boundary.test.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json',
  },
  comments_event_projection: {
    package_script: 'verify:blog:comments-event-projection',
    test_package_script: 'test:verify:blog:comments-event-projection',
    verifier: 'scripts/verify/verify-blog-comments-event-projection.mjs',
    self_test: 'scripts/verify/verify-blog-comments-event-projection.test.mjs',
    unit_test: 'crates/rustok-blog/src/services/comment_projection.rs',
    postgres_test: 'crates/rustok-blog/tests/comment_projection_postgres_test.rs',
    restart_test: 'crates/rustok-blog/tests/comment_projection_restart_postgres_test.rs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-comments-event-projection.json',
  },
  comments_duplicate_delivery_race: {
    package_script: 'verify:blog:comments-duplicate-delivery-race',
    test_package_script: 'test:verify:blog:comments-duplicate-delivery-race',
    verifier: 'scripts/verify/verify-blog-comments-duplicate-delivery-race.mjs',
    self_test: 'scripts/verify/verify-blog-comments-duplicate-delivery-race.test.mjs',
    postgres_test: 'crates/rustok-blog/tests/comment_projection_duplicate_race_postgres_test.rs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-comments-duplicate-delivery-race.json',
  },
  comments_dispatcher_duplicate_delivery: {
    package_script: 'verify:blog:comments-dispatcher-duplicate-delivery',
    test_package_script: 'test:verify:blog:comments-dispatcher-duplicate-delivery',
    verifier: 'scripts/verify/verify-blog-comments-dispatcher-duplicate-delivery.mjs',
    self_test: 'scripts/verify/verify-blog-comments-dispatcher-duplicate-delivery.test.mjs',
    postgres_test: 'crates/rustok-blog/tests/comment_projection_dispatcher_duplicate_postgres_test.rs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-comments-dispatcher-duplicate-delivery.json',
  },
  category_search_reindex: {
    package_script: 'verify:blog:category-search-reindex',
    test_package_script: 'test:verify:blog:category-search-reindex',
    verifier: 'scripts/verify/verify-blog-category-search-reindex.mjs',
    self_test: 'scripts/verify/verify-blog-category-search-reindex.test.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-category-search-reindex-contract.json',
  },
  graphql_rate_limit: {
    package_script: 'verify:blog:graphql-rate-limit',
    test_package_script: 'test:verify:blog:graphql-rate-limit',
    verifier: 'scripts/verify/verify-blog-graphql-rate-limit.mjs',
    self_test: 'scripts/verify/verify-blog-graphql-rate-limit.test.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-graphql-rate-limit-runtime-harness.json',
  },
  graphql_richtext_boundary: {
    package_script: 'verify:blog:graphql-richtext-boundary',
    test_package_script: 'test:verify:blog:graphql-richtext-boundary',
    verifier: 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs',
    self_test: 'scripts/verify/verify-blog-graphql-richtext-boundary.test.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-graphql-richtext-boundary.json',
  },
  ai_richtext_boundary: {
    package_script: 'verify:blog:ai-richtext-boundary',
    test_package_script: 'test:verify:blog:ai-richtext-boundary',
    verifier: 'scripts/verify/verify-blog-ai-richtext-boundary.mjs',
    self_test: 'scripts/verify/verify-blog-ai-richtext-boundary.test.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-ai-richtext-boundary.json',
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
  'npm run test:verify:blog:comments-port-boundary',
  'npm run test:verify:blog:comments-event-projection',
  'npm run test:verify:blog:comments-duplicate-delivery-race',
  'npm run test:verify:blog:comments-dispatcher-duplicate-delivery',
  'npm run test:verify:blog:category-search-reindex',
  'npm run test:verify:blog:graphql-rate-limit',
  'npm run test:verify:blog:graphql-richtext-boundary',
  'npm run test:verify:blog:ai-richtext-boundary',
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
      gate?.evidence !== expectedGate.evidence ||
      gate?.unit_test !== expectedGate.unit_test ||
      gate?.postgres_test !== expectedGate.postgres_test ||
      gate?.restart_test !== expectedGate.restart_test
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

    for (const filePath of [
      expectedGate.verifier,
      expectedGate.self_test,
      expectedGate.evidence,
      expectedGate.unit_test,
      expectedGate.postgres_test,
      expectedGate.restart_test,
    ].filter(Boolean)) {
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
