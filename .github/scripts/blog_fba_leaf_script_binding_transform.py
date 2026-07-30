from pathlib import Path
import json


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one replacement, found {count}: {old[:160]!r}")
    target.write_text(text.replace(old, new, 1))


registry_path = "crates/rustok-blog/contracts/blog-fba-registry.json"
policy_path = "scripts/verify/blog-fba-verification-chain.mjs"
verifier_path = "scripts/verify/verify-blog-fba.mjs"
test_path = "scripts/verify/verify-blog-fba.test.mjs"
plan_path = "crates/rustok-blog/docs/implementation-plan.md"

replace_once(registry_path, '  "schema_version": 3,', '  "schema_version": 4,')
replace_once(verifier_path, "if (registry.schema_version !== 3) fail('registry schema_version drift');", "if (registry.schema_version !== 4) fail('registry schema_version drift');")

source_gates = {
    "admin_boundary": "verify:blog:admin-boundary",
    "storefront_boundary": "verify:blog:storefront-boundary",
    "graphql_richtext_boundary": "verify:blog:graphql-richtext-boundary",
    "richtext_offline_backfill": "verify:blog:richtext-offline-backfill",
    "forum_ui_ownership": "verify:blog:forum-ui-ownership",
}

for gate_name, package_script in source_gates.items():
    replace_once(
        registry_path,
        f'      "{gate_name}": {{\n        "verifier":',
        f'      "{gate_name}": {{\n        "package_script": "{package_script}",\n        "verifier":',
    )
    replace_once(
        policy_path,
        f'  {gate_name}: {{\n    verifier:',
        f"  {gate_name}: {{\n    package_script: '{package_script}',\n    verifier:",
    )

replace_once(
    policy_path,
    """    if (gate?.verifier !== expectedGate.verifier || gate?.evidence !== expectedGate.evidence) {
      failures.push(`registry source gate ${gateName} path drift`);
    }
    for (const filePath of [expectedGate.verifier, expectedGate.evidence]) {""",
    """    if (gate?.package_script !== expectedGate.package_script) {
      failures.push(`registry source gate ${gateName} package script drift`);
    }
    if (gate?.verifier !== expectedGate.verifier || gate?.evidence !== expectedGate.evidence) {
      failures.push(`registry source gate ${gateName} path drift`);
    }

    const leafCommand = packageJson?.scripts?.[expectedGate.package_script];
    const expectedLeafCommand = `node ${expectedGate.verifier}`;
    if (typeof leafCommand !== 'string') {
      failures.push(`package.json missing source gate script ${expectedGate.package_script}`);
    } else if (leafCommand !== expectedLeafCommand) {
      failures.push(`package source gate ${gateName} command drift`);
    }

    for (const filePath of [expectedGate.verifier, expectedGate.evidence]) {""",
)

replace_once(
    test_path,
    """function canonicalPackageJson() {
  return {
    scripts: {
      'verify:blog:fba': BLOG_FBA_VERIFICATION_STEPS.join(' && '),
      'test:verify:blog:fba': BLOG_FBA_SELF_TEST_COMMAND,
    },
  };
}""",
    """function canonicalPackageJson() {
  const scripts = {
    'verify:blog:fba': BLOG_FBA_VERIFICATION_STEPS.join(' && '),
    'test:verify:blog:fba': BLOG_FBA_SELF_TEST_COMMAND,
  };
  for (const gate of Object.values(BLOG_FBA_SOURCE_GATES)) {
    scripts[gate.package_script] = `node ${gate.verifier}`;
  }
  return { scripts };
}""",
)

replace_once(
    test_path,
    """test('Blog FBA verification-chain policy rejects source-gate path drift', () => {
  const registry = canonicalRegistry();
  registry.verification_chain.source_gates.storefront_boundary.evidence = 'wrong/storefront-evidence.json';
  assert.ok(failures({ registry }).includes('registry source gate storefront_boundary path drift'));
});

""",
    """test('Blog FBA verification-chain policy rejects source-gate path drift', () => {
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

""",
)

replace_once(
    plan_path,
    """The exact-chain policy now lives in a pure source module shared by the aggregate
verifier and its self-regression fixture. Negative cases cover missing storefront
execution, package/registry order drift, source-gate path drift, missing files, and
self-test binding drift without requiring product compilation or database access.""",
    """The exact-chain policy now lives in a pure source module shared by the aggregate
verifier and its self-regression fixture. Negative cases cover missing storefront
execution, package/registry order drift, source-gate path drift, missing files, and
self-test binding drift without requiring product compilation or database access.
Every registered leaf gate also binds its npm script to the exact verifier command;
renaming, removing, or repointing a `verify:blog:*` script now fails the aggregate
policy instead of leaving a no-op behind the correct aggregate step name.""",
)

replace_once(
    plan_path,
    """- Blog FBA source-gate chain: `source_verified_no_compile`; registry schema v3
  locks the exact package order, source-gate paths, and aggregate self-test binding
  for admin, storefront, GraphQL richtext, offline backfill, Forum ownership, and
  consumer runtime-order gates.""",
    """- Blog FBA source-gate chain: `source_verified_no_compile`; registry schema v4
  locks the exact package order, leaf npm-script-to-verifier commands, source-gate
  paths, and aggregate self-test binding for admin, storefront, GraphQL richtext,
  offline backfill, Forum ownership, and consumer runtime-order gates.""",
)

replace_once(
    plan_path,
    """29. Extracted the exact Blog FBA chain policy into a pure module and added an
    aggregate self-regression fixture for missing storefront execution, package or
    registry order drift, source-path drift, missing files, and self-test binding.

## Next results""",
    """29. Extracted the exact Blog FBA chain policy into a pure module and added an
    aggregate self-regression fixture for missing storefront execution, package or
    registry order drift, source-path drift, missing files, and self-test binding.
30. Bound every registered Blog FBA leaf npm script to its exact verifier command
    and extended the aggregate policy fixture to reject missing, repointed, or
    registry-renamed source-gate scripts.

## Next results""",
)

registry = json.loads(Path(registry_path).read_text())
package = json.loads(Path('package.json').read_text())
if registry['schema_version'] != 4:
    raise RuntimeError('registry schema version did not advance to 4')
for gate_name, expected_script in source_gates.items():
    gate = registry['verification_chain']['source_gates'][gate_name]
    if gate['package_script'] != expected_script:
        raise RuntimeError(f'{gate_name}: registry package script drift')
    expected_command = f"node {gate['verifier']}"
    if package['scripts'].get(expected_script) != expected_command:
        raise RuntimeError(f'{gate_name}: package command is not bound to verifier')
