from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one replacement, found {count}: {old[:160]!r}")
    target.write_text(text.replace(old, new, 1))


def write_new(path: str, content: str) -> None:
    target = Path(path)
    if target.exists():
        raise RuntimeError(f"{path}: expected new file")
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)


policy_module = "import fs from 'node:fs';\n\nexport const BLOG_FBA_VERIFICATION_STEPS = [\n  'node scripts/verify/verify-blog-fba.mjs',\n  'npm run verify:blog:admin-boundary',\n  'npm run verify:blog:storefront-boundary',\n  'npm run verify:blog:graphql-richtext-boundary',\n  'npm run verify:blog:richtext-offline-backfill',\n  'npm run verify:blog:forum-ui-ownership',\n  'node scripts/verify/verify-consumer-fba-runtime-order.mjs',\n];\n\nexport const BLOG_FBA_SOURCE_GATES = {\n  admin_boundary: {\n    verifier: 'scripts/verify/verify-blog-admin-boundary.mjs',\n    evidence: 'crates/rustok-blog/contracts/evidence/blog-admin-richtext-boundary.json',\n  },\n  storefront_boundary: {\n    verifier: 'scripts/verify/verify-blog-storefront-boundary.mjs',\n    evidence: 'crates/rustok-blog/contracts/evidence/blog-storefront-richtext-view.json',\n  },\n  graphql_richtext_boundary: {\n    verifier: 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs',\n    evidence: 'crates/rustok-blog/contracts/evidence/blog-graphql-richtext-boundary.json',\n  },\n  richtext_offline_backfill: {\n    verifier: 'scripts/verify/verify-blog-richtext-offline-backfill.mjs',\n    evidence: 'crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json',\n  },\n  forum_ui_ownership: {\n    verifier: 'scripts/verify/verify-blog-forum-ui-ownership.mjs',\n    evidence: 'crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json',\n  },\n};\n\nexport const BLOG_FBA_SELF_TEST = 'scripts/verify/verify-blog-fba.test.mjs';\nexport const BLOG_FBA_SELF_TEST_COMMAND = `node ${BLOG_FBA_SELF_TEST}`;\n\nfunction sameList(actual, expected) {\n  return JSON.stringify(actual) === JSON.stringify(expected);\n}\n\nfunction sameSet(actual, expected) {\n  return [...actual].sort().join('|') === [...expected].sort().join('|');\n}\n\nexport function collectBlogFbaVerificationChainFailures({\n  registry,\n  packageJson,\n  existsSync = fs.existsSync,\n} = {}) {\n  const failures = [];\n  const chain = registry?.verification_chain;\n\n  if (chain?.package_script !== 'verify:blog:fba') {\n    failures.push('verification chain package script drift');\n  }\n  if (chain?.self_test !== BLOG_FBA_SELF_TEST) {\n    failures.push('verification chain self-test path drift');\n  }\n  if (!sameList(chain?.steps ?? [], BLOG_FBA_VERIFICATION_STEPS)) {\n    failures.push('registry verification chain steps drift');\n  }\n\n  const packageScript = packageJson?.scripts?.['verify:blog:fba'];\n  if (typeof packageScript !== 'string') {\n    failures.push('package.json missing verify:blog:fba script');\n  } else if (!sameList(packageScript.split(' && '), BLOG_FBA_VERIFICATION_STEPS)) {\n    failures.push('package verification chain steps drift');\n  }\n\n  const packageSelfTest = packageJson?.scripts?.['test:verify:blog:fba'];\n  if (packageSelfTest !== BLOG_FBA_SELF_TEST_COMMAND) {\n    failures.push('package Blog FBA self-test script drift');\n  }\n\n  const sourceGates = chain?.source_gates ?? {};\n  if (!sameSet(Object.keys(sourceGates), Object.keys(BLOG_FBA_SOURCE_GATES))) {\n    failures.push('registry source gate names drift');\n  }\n\n  for (const [gateName, expectedGate] of Object.entries(BLOG_FBA_SOURCE_GATES)) {\n    const gate = sourceGates[gateName];\n    if (gate?.verifier !== expectedGate.verifier || gate?.evidence !== expectedGate.evidence) {\n      failures.push(`registry source gate ${gateName} path drift`);\n    }\n    for (const filePath of [expectedGate.verifier, expectedGate.evidence]) {\n      if (!existsSync(filePath)) {\n        failures.push(`registry source gate ${gateName} missing ${filePath}`);\n      }\n    }\n  }\n\n  if (!existsSync(BLOG_FBA_SELF_TEST)) {\n    failures.push(`verification chain missing self-test ${BLOG_FBA_SELF_TEST}`);\n  }\n\n  return failures;\n}\n"
self_test = "#!/usr/bin/env node\n\nimport test from 'node:test';\nimport assert from 'node:assert/strict';\nimport {\n  BLOG_FBA_SELF_TEST,\n  BLOG_FBA_SELF_TEST_COMMAND,\n  BLOG_FBA_SOURCE_GATES,\n  BLOG_FBA_VERIFICATION_STEPS,\n  collectBlogFbaVerificationChainFailures,\n} from './blog-fba-verification-chain.mjs';\n\nfunction clone(value) {\n  return JSON.parse(JSON.stringify(value));\n}\n\nfunction canonicalRegistry() {\n  return {\n    verification_chain: {\n      package_script: 'verify:blog:fba',\n      self_test: BLOG_FBA_SELF_TEST,\n      steps: [...BLOG_FBA_VERIFICATION_STEPS],\n      source_gates: clone(BLOG_FBA_SOURCE_GATES),\n    },\n  };\n}\n\nfunction canonicalPackageJson() {\n  return {\n    scripts: {\n      'verify:blog:fba': BLOG_FBA_VERIFICATION_STEPS.join(' && '),\n      'test:verify:blog:fba': BLOG_FBA_SELF_TEST_COMMAND,\n    },\n  };\n}\n\nfunction canonicalExistingPaths() {\n  return new Set([\n    BLOG_FBA_SELF_TEST,\n    ...Object.values(BLOG_FBA_SOURCE_GATES).flatMap((gate) => [gate.verifier, gate.evidence]),\n  ]);\n}\n\nfunction failures({ registry = canonicalRegistry(), packageJson = canonicalPackageJson(), existingPaths = canonicalExistingPaths() } = {}) {\n  return collectBlogFbaVerificationChainFailures({\n    registry,\n    packageJson,\n    existsSync: (filePath) => existingPaths.has(filePath),\n  });\n}\n\ntest('Blog FBA verification-chain policy accepts the canonical registry and package scripts', () => {\n  assert.deepEqual(failures(), []);\n});\n\ntest('Blog FBA verification-chain policy rejects removal of the storefront step', () => {\n  const registry = canonicalRegistry();\n  registry.verification_chain.steps = registry.verification_chain.steps.filter(\n    (step) => step !== 'npm run verify:blog:storefront-boundary',\n  );\n  assert.ok(failures({ registry }).includes('registry verification chain steps drift'));\n});\n\ntest('Blog FBA verification-chain policy rejects package and registry order drift', () => {\n  const packageJson = canonicalPackageJson();\n  packageJson.scripts['verify:blog:fba'] = BLOG_FBA_VERIFICATION_STEPS\n    .filter((step) => step !== 'npm run verify:blog:storefront-boundary')\n    .join(' && ');\n  assert.ok(failures({ packageJson }).includes('package verification chain steps drift'));\n});\n\ntest('Blog FBA verification-chain policy rejects source-gate path drift', () => {\n  const registry = canonicalRegistry();\n  registry.verification_chain.source_gates.storefront_boundary.evidence = 'wrong/storefront-evidence.json';\n  assert.ok(failures({ registry }).includes('registry source gate storefront_boundary path drift'));\n});\n\ntest('Blog FBA verification-chain policy rejects a missing registered source-gate file', () => {\n  const existingPaths = canonicalExistingPaths();\n  existingPaths.delete(BLOG_FBA_SOURCE_GATES.storefront_boundary.verifier);\n  assert.ok(\n    failures({ existingPaths }).includes(\n      `registry source gate storefront_boundary missing ${BLOG_FBA_SOURCE_GATES.storefront_boundary.verifier}`,\n    ),\n  );\n});\n\ntest('Blog FBA verification-chain policy rejects self-test path drift', () => {\n  const registry = canonicalRegistry();\n  registry.verification_chain.self_test = 'scripts/verify/wrong-blog-fba.test.mjs';\n  assert.ok(failures({ registry }).includes('verification chain self-test path drift'));\n});\n\ntest('Blog FBA verification-chain policy rejects package self-test script drift', () => {\n  const packageJson = canonicalPackageJson();\n  packageJson.scripts['test:verify:blog:fba'] = 'node scripts/verify/wrong-blog-fba.test.mjs';\n  assert.ok(failures({ packageJson }).includes('package Blog FBA self-test script drift'));\n});\n\ntest('Blog FBA verification-chain policy rejects a missing self-test file', () => {\n  const existingPaths = canonicalExistingPaths();\n  existingPaths.delete(BLOG_FBA_SELF_TEST);\n  assert.ok(\n    failures({ existingPaths }).includes(`verification chain missing self-test ${BLOG_FBA_SELF_TEST}`),\n  );\n});\n"

write_new("scripts/verify/blog-fba-verification-chain.mjs", policy_module)
write_new("scripts/verify/verify-blog-fba.test.mjs", self_test)

verifier_path = "scripts/verify/verify-blog-fba.mjs"
replace_once(
    verifier_path,
    "import fs from 'node:fs';\n",
    "import fs from 'node:fs';\nimport { collectBlogFbaVerificationChainFailures } from './blog-fba-verification-chain.mjs';\n",
)
replace_once(
    verifier_path,
    """function sameList(actual, expected, label) {
  const a = JSON.stringify(actual);
  const e = JSON.stringify(expected);
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}

""",
    "",
)
replace_once(
    verifier_path,
    """const expectedVerificationSteps = [
  'node scripts/verify/verify-blog-fba.mjs',
  'npm run verify:blog:admin-boundary',
  'npm run verify:blog:storefront-boundary',
  'npm run verify:blog:graphql-richtext-boundary',
  'npm run verify:blog:richtext-offline-backfill',
  'npm run verify:blog:forum-ui-ownership',
  'node scripts/verify/verify-consumer-fba-runtime-order.mjs',
];
const expectedSourceGates = {
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
""",
    "",
)
replace_once(verifier_path, "if (registry.schema_version !== 2) fail('registry schema_version drift');", "if (registry.schema_version !== 3) fail('registry schema_version drift');")
replace_once(
    verifier_path,
    """if (registry.verification_chain?.package_script !== 'verify:blog:fba') fail('verification chain package script drift');
sameList(registry.verification_chain?.steps ?? [], expectedVerificationSteps, 'registry verification chain steps');
const packageScript = packageJson.scripts?.['verify:blog:fba'];
if (typeof packageScript !== 'string') fail('package.json missing verify:blog:fba script');
sameList(packageScript.split(' && '), expectedVerificationSteps, 'package verification chain steps');
const sourceGates = registry.verification_chain?.source_gates ?? {};
sameSet(Object.keys(sourceGates), Object.keys(expectedSourceGates), 'registry source gate names');
for (const [gateName, expectedGate] of Object.entries(expectedSourceGates)) {
  const gate = sourceGates[gateName];
  if (gate?.verifier !== expectedGate.verifier || gate?.evidence !== expectedGate.evidence) {
    fail(`registry source gate ${gateName} path drift`);
  }
  for (const filePath of [expectedGate.verifier, expectedGate.evidence]) {
    if (!fs.existsSync(filePath)) fail(`registry source gate ${gateName} missing ${filePath}`);
  }
}
""",
    """for (const failure of collectBlogFbaVerificationChainFailures({
  registry,
  packageJson,
  existsSync: fs.existsSync,
})) {
  fail(failure);
}
""",
)

registry_path = "crates/rustok-blog/contracts/blog-fba-registry.json"
replace_once(registry_path, '"schema_version": 2,', '"schema_version": 3,')
replace_once(
    registry_path,
    '    "package_script": "verify:blog:fba",\n    "steps": [',
    '    "package_script": "verify:blog:fba",\n    "self_test": "scripts/verify/verify-blog-fba.test.mjs",\n    "steps": [',
)

package_path = "package.json"
replace_once(
    package_path,
    '    "verify:blog:fba": "node scripts/verify/verify-blog-fba.mjs && npm run verify:blog:admin-boundary && npm run verify:blog:storefront-boundary && npm run verify:blog:graphql-richtext-boundary && npm run verify:blog:richtext-offline-backfill && npm run verify:blog:forum-ui-ownership && node scripts/verify/verify-consumer-fba-runtime-order.mjs",',
    '    "verify:blog:fba": "node scripts/verify/verify-blog-fba.mjs && npm run verify:blog:admin-boundary && npm run verify:blog:storefront-boundary && npm run verify:blog:graphql-richtext-boundary && npm run verify:blog:richtext-offline-backfill && npm run verify:blog:forum-ui-ownership && node scripts/verify/verify-consumer-fba-runtime-order.mjs",\n    "test:verify:blog:fba": "node scripts/verify/verify-blog-fba.test.mjs",',
)

plan_path = "crates/rustok-blog/docs/implementation-plan.md"
replace_once(
    plan_path,
    """The Blog FBA source-gate chain is now registry-locked. The package command must
preserve the exact admin, storefront, GraphQL richtext, offline backfill, Forum UI
ownership, and consumer runtime-order sequence; the FBA verifier also checks every
registered verifier/evidence path. Storefront can no longer disappear from the
aggregate gate while the module still claims `core_transport_ui` readiness.
""",
    """The Blog FBA source-gate chain is now registry-locked. The package command must
preserve the exact admin, storefront, GraphQL richtext, offline backfill, Forum UI
ownership, and consumer runtime-order sequence; the FBA verifier also checks every
registered verifier/evidence path. Storefront can no longer disappear from the
aggregate gate while the module still claims `core_transport_ui` readiness.
The exact-chain policy now lives in a pure source module shared by the aggregate
verifier and its self-regression fixture. Negative cases cover missing storefront
execution, package/registry order drift, source-gate path drift, missing files, and
self-test binding drift without requiring product compilation or database access.
""",
)
replace_once(
    plan_path,
    """- Blog FBA source-gate chain: `source_verified_no_compile`; registry schema v2
  locks the exact package order and requires admin, storefront, GraphQL richtext,
  offline backfill, Forum ownership, and consumer runtime-order gates.
""",
    """- Blog FBA source-gate chain: `source_verified_no_compile`; registry schema v3
  locks the exact package order, source-gate paths, and aggregate self-test binding
  for admin, storefront, GraphQL richtext, offline backfill, Forum ownership, and
  consumer runtime-order gates.
""",
)
replace_once(
    plan_path,
    """28. Locked the Blog FBA package command to registry schema v2 and restored the
    missing storefront boundary gate, with exact ordered-step and source-path
    validation in the aggregate verifier.
""",
    """28. Locked the Blog FBA package command to registry schema v2 and restored the
    missing storefront boundary gate, with exact ordered-step and source-path
    validation in the aggregate verifier.
29. Extracted the exact Blog FBA chain policy into a pure module and added an
    aggregate self-regression fixture for missing storefront execution, package or
    registry order drift, source-path drift, missing files, and self-test binding.
""",
)
