import fs from 'node:fs';

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
    verifier: 'scripts/verify/verify-blog-admin-boundary.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-admin-richtext-boundary.json',
  },
  storefront_boundary: {
    verifier: 'scripts/verify/verify-blog-storefront-boundary.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-storefront-richtext-view.json',
  },
  graphql_richtext_boundary: {
    verifier: 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-graphql-richtext-boundary.json',
  },
  richtext_offline_backfill: {
    verifier: 'scripts/verify/verify-blog-richtext-offline-backfill.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json',
  },
  forum_ui_ownership: {
    verifier: 'scripts/verify/verify-blog-forum-ui-ownership.mjs',
    evidence: 'crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json',
  },
};

export const BLOG_FBA_SELF_TEST = 'scripts/verify/verify-blog-fba.test.mjs';
export const BLOG_FBA_SELF_TEST_COMMAND = `node ${BLOG_FBA_SELF_TEST}`;

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

  const packageScript = packageJson?.scripts?.['verify:blog:fba'];
  if (typeof packageScript !== 'string') {
    failures.push('package.json missing verify:blog:fba script');
  } else if (!sameList(packageScript.split(' && '), BLOG_FBA_VERIFICATION_STEPS)) {
    failures.push('package verification chain steps drift');
  }

  const packageSelfTest = packageJson?.scripts?.['test:verify:blog:fba'];
  if (packageSelfTest !== BLOG_FBA_SELF_TEST_COMMAND) {
    failures.push('package Blog FBA self-test script drift');
  }

  const sourceGates = chain?.source_gates ?? {};
  if (!sameSet(Object.keys(sourceGates), Object.keys(BLOG_FBA_SOURCE_GATES))) {
    failures.push('registry source gate names drift');
  }

  for (const [gateName, expectedGate] of Object.entries(BLOG_FBA_SOURCE_GATES)) {
    const gate = sourceGates[gateName];
    if (gate?.verifier !== expectedGate.verifier || gate?.evidence !== expectedGate.evidence) {
      failures.push(`registry source gate ${gateName} path drift`);
    }
    for (const filePath of [expectedGate.verifier, expectedGate.evidence]) {
      if (!existsSync(filePath)) {
        failures.push(`registry source gate ${gateName} missing ${filePath}`);
      }
    }
  }

  if (!existsSync(BLOG_FBA_SELF_TEST)) {
    failures.push(`verification chain missing self-test ${BLOG_FBA_SELF_TEST}`);
  }

  return failures;
}
