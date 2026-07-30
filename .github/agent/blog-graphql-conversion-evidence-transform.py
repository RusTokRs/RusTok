from pathlib import Path
import json

ROOT = Path('.')

VERIFIER = r'''#!/usr/bin/env node
import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import { join, relative, sep } from 'node:path';

const ROOT = 'crates/rustok-blog/src/graphql';
const TYPES = `${ROOT}/types.rs`;
const MUTATION = `${ROOT}/mutation.rs`;
const CREATE_TEST = 'crates/rustok-blog/tests/graphql_create_post_input_conversion_test.rs';
const EVIDENCE = 'crates/rustok-blog/contracts/evidence/blog-graphql-richtext-boundary.json';
const GUARDRAIL = 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs';
const GUARDRAIL_TEST = 'scripts/verify/verify-blog-graphql-richtext-boundary.test.mjs';

const CREATE_MARKERS = [
  'create_post_input_conversion_preserves_canonical_content',
  'RichTextDocument::single_paragraph',
  'let domain: DomainCreatePostInput = input.into();',
  'assert_eq!(domain.content, canonical)',
];
const UPDATE_MARKERS = [
  'update_post_input_conversion_preserves_canonical_content',
  'RichTextDocument::single_paragraph',
  'let domain: DomainUpdatePostInput = input.into();',
  'assert_eq!(domain.content, Some(canonical))',
];
const CONVERSION_SCOPES = [
  'impl From<CreatePostInput> for DomainCreatePostInput',
  'impl From<UpdatePostInput> for DomainUpdatePostInput',
];
const EXPECTED_CONVERSION_TESTS = {
  create: { file: CREATE_TEST, markers: CREATE_MARKERS },
  update: { file: TYPES, markers: UPDATE_MARKERS },
};

async function rustFiles(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  const out = [];
  for (const entry of entries) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...await rustFiles(path));
    else if (entry.isFile() && entry.name.endsWith('.rs')) out.push(path.split(sep).join('/'));
  }
  return out;
}
function between(source, start, end) {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  assert.notEqual(from, -1, `missing ${start}`);
  assert.notEqual(to, -1, `missing ${end}`);
  return source.slice(from, to);
}
function requireMarkers(source, markers, label) {
  for (const marker of markers) assert.ok(source.includes(marker), `missing ${label} ${marker}`);
}

const paths = await rustFiles(ROOT);
const sources = new Map(await Promise.all(paths.map(async path => [path, await readFile(path, 'utf8')])));
const types = sources.get(TYPES);
const mutation = sources.get(MUTATION);
const createTest = await readFile(CREATE_TEST, 'utf8');
const evidence = JSON.parse(await readFile(EVIDENCE, 'utf8'));

assert.equal(evidence.schema_version, 3, 'GraphQL richtext evidence schema drift');
assert.equal(evidence.owner, 'rustok-blog', 'GraphQL richtext evidence owner drift');
assert.equal(evidence.boundary, 'graphql-post-richtext', 'GraphQL richtext evidence boundary drift');
assert.equal(
  evidence.status,
  'implemented_source_verified_no_compile',
  'GraphQL richtext evidence status drift',
);
assert.deepEqual(evidence.canonical_contract, {
  write: 'rustok_api::RichTextDocument',
  read: 'rustok_api::RichTextView',
  plain_text: 'server-derived',
  resolver_conversion: 'typed input.into() delegation with direct content mapping',
}, 'GraphQL richtext canonical contract drift');
assert.equal(evidence.scan_scope, 'crates/rustok-blog/src/graphql/**/*.rs', 'GraphQL scan scope drift');
assert.deepEqual(evidence.legacy_adapter_fields, [], 'legacy GraphQL adapter fields returned');
assert.deepEqual(evidence.legacy_adapter_files, [], 'legacy GraphQL adapter files returned');
assert.deepEqual(evidence.conversion_owner, {
  file: TYPES,
  scopes: CONVERSION_SCOPES,
  reason: 'target-only typed GraphQL-to-owner conversion',
}, 'GraphQL conversion-owner evidence drift');
assert.deepEqual(
  evidence.conversion_tests,
  EXPECTED_CONVERSION_TESTS,
  'GraphQL create/update conversion-test evidence drift',
);
assert.equal(evidence.guardrail, GUARDRAIL, 'GraphQL guardrail path drift');
assert.equal(evidence.guardrail_test, GUARDRAIL_TEST, 'GraphQL guardrail fixture path drift');

for (const marker of [
  'pub content: RichTextView',
  'pub content_plain_text: String',
  'pub content: RichTextDocument',
  'pub content: Option<RichTextDocument>',
  ...CONVERSION_SCOPES,
]) assert.ok(types.includes(marker), `missing target marker ${marker}`);
for (const [file, source] of sources) {
  for (const forbidden of ['pub body:', 'pub body_format:', 'pub content_json:', 'CONTENT_FORMAT_MARKDOWN', 'rt_json', 'markdown_body', 'raw_content_json']) {
    assert.ok(!source.includes(forbidden), `forbidden ${forbidden} in ${relative('.', file)}`);
  }
}

const createConversion = between(
  types,
  CONVERSION_SCOPES[0],
  CONVERSION_SCOPES[1],
);
const updateConversion = between(
  types,
  CONVERSION_SCOPES[1],
  '#[cfg(test)]',
);
assert.match(
  createConversion,
  /\bcontent:\s*input\.content,/, 
  'create conversion must map canonical content directly',
);
assert.match(
  updateConversion,
  /\bcontent:\s*input\.content,/, 
  'update conversion must map canonical content directly',
);

for (const source of [
  between(mutation, 'async fn create_post(', 'async fn update_post('),
  between(mutation, 'async fn update_post(', 'async fn delete_post('),
]) {
  assert.ok(source.includes('input.into()'), 'resolver must delegate through input.into()');
  assert.ok(!source.includes('input.content'), 'resolver must not map content manually');
}
requireMarkers(createTest, CREATE_MARKERS, 'create conversion coverage');
requireMarkers(types, UPDATE_MARKERS, 'update conversion coverage');

console.log('Blog GraphQL target-only richtext boundary guardrail passed.');
'''

SELF_TEST = r'''#!/usr/bin/env node
import assert from 'node:assert/strict';
import { copyFile, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = resolve('scripts/verify/verify-blog-graphql-richtext-boundary.mjs');
const createTestPath = 'crates/rustok-blog/tests/graphql_create_post_input_conversion_test.rs';
const typesPath = 'crates/rustok-blog/src/graphql/types.rs';

const createMarkers = [
  'create_post_input_conversion_preserves_canonical_content',
  'RichTextDocument::single_paragraph',
  'let domain: DomainCreatePostInput = input.into();',
  'assert_eq!(domain.content, canonical)',
];
const updateMarkers = [
  'update_post_input_conversion_preserves_canonical_content',
  'RichTextDocument::single_paragraph',
  'let domain: DomainUpdatePostInput = input.into();',
  'assert_eq!(domain.content, Some(canonical))',
];

function canonicalEvidence() {
  return {
    schema_version: 3,
    owner: 'rustok-blog',
    boundary: 'graphql-post-richtext',
    status: 'implemented_source_verified_no_compile',
    canonical_contract: {
      write: 'rustok_api::RichTextDocument',
      read: 'rustok_api::RichTextView',
      plain_text: 'server-derived',
      resolver_conversion: 'typed input.into() delegation with direct content mapping',
    },
    scan_scope: 'crates/rustok-blog/src/graphql/**/*.rs',
    legacy_adapter_fields: [],
    legacy_adapter_files: [],
    conversion_owner: {
      file: typesPath,
      scopes: [
        'impl From<CreatePostInput> for DomainCreatePostInput',
        'impl From<UpdatePostInput> for DomainUpdatePostInput',
      ],
      reason: 'target-only typed GraphQL-to-owner conversion',
    },
    conversion_tests: {
      create: { file: createTestPath, markers: createMarkers },
      update: { file: typesPath, markers: updateMarkers },
    },
    guardrail: 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs',
    guardrail_test: 'scripts/verify/verify-blog-graphql-richtext-boundary.test.mjs',
    exit_condition: 'Source cutover complete; compile, migration, and runtime execution remain maintainer-owned.',
  };
}

const canonicalTypes = `
pub content: RichTextView,
pub content_plain_text: String,
pub content: RichTextDocument,
pub content: Option<RichTextDocument>,
impl From<CreatePostInput> for DomainCreatePostInput {
  fn from(input: CreatePostInput) -> Self { Self { content: input.content, } }
}
impl From<UpdatePostInput> for DomainUpdatePostInput {
  fn from(input: UpdatePostInput) -> Self { Self { content: input.content, } }
}
#[cfg(test)]
mod tests {
  fn update_post_input_conversion_preserves_canonical_content() {
    let canonical = RichTextDocument::single_paragraph("canonical update");
    let domain: DomainUpdatePostInput = input.into();
    assert_eq!(domain.content, Some(canonical));
  }
}
`;
const canonicalMutation = `
async fn create_post(input: CreatePostInput) { service.create_post(input.into()); }
async fn update_post(input: UpdatePostInput) { service.update_post(input.into()); }
async fn delete_post() {}
`;
const canonicalCreateTest = `
fn create_post_input_conversion_preserves_canonical_content() {
  let canonical = RichTextDocument::single_paragraph("canonical");
  let domain: DomainCreatePostInput = input.into();
  assert_eq!(domain.content, canonical);
}
`;

async function run({
  types = canonicalTypes,
  mutation = canonicalMutation,
  createTest = canonicalCreateTest,
  evidence = canonicalEvidence(),
} = {}) {
  const root = await mkdtemp(join(tmpdir(), 'blog-gql-target-'));
  try {
    await mkdir(join(root, 'scripts/verify'), { recursive: true });
    await mkdir(join(root, 'crates/rustok-blog/src/graphql'), { recursive: true });
    await mkdir(join(root, 'crates/rustok-blog/tests'), { recursive: true });
    await mkdir(join(root, 'crates/rustok-blog/contracts/evidence'), { recursive: true });
    await copyFile(verifier, join(root, 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs'));
    await writeFile(join(root, typesPath), types);
    await writeFile(join(root, 'crates/rustok-blog/src/graphql/mutation.rs'), mutation);
    await writeFile(join(root, createTestPath), createTest);
    await writeFile(
      join(root, 'crates/rustok-blog/contracts/evidence/blog-graphql-richtext-boundary.json'),
      JSON.stringify(evidence, null, 2),
    );
    return spawnSync(process.execPath, ['scripts/verify/verify-blog-graphql-richtext-boundary.mjs'], {
      cwd: root,
      encoding: 'utf8',
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

let result = await run();
assert.equal(result.status, 0, result.stderr);

result = await run({ types: `${canonicalTypes}\npub body_format: String,` });
assert.notEqual(result.status, 0);

result = await run({
  mutation: canonicalMutation.replace('service.create_post(input.into())', 'service.create_post(input.content)'),
});
assert.notEqual(result.status, 0);

result = await run({
  types: canonicalTypes.replace('Self { content: input.content, }', 'Self { content: canonical, }'),
});
assert.notEqual(result.status, 0);
assert.match(result.stderr, /create conversion must map canonical content directly/);

result = await run({
  types: canonicalTypes.replace(
    'fn from(input: UpdatePostInput) -> Self { Self { content: input.content, } }',
    'fn from(input: UpdatePostInput) -> Self { Self { content: None, } }',
  ),
});
assert.notEqual(result.status, 0);
assert.match(result.stderr, /update conversion must map canonical content directly/);

result = await run({
  createTest: canonicalCreateTest.replace('assert_eq!(domain.content, canonical);', ''),
});
assert.notEqual(result.status, 0);
assert.match(result.stderr, /create conversion coverage/);

result = await run({
  types: canonicalTypes.replace('assert_eq!(domain.content, Some(canonical));', ''),
});
assert.notEqual(result.status, 0);
assert.match(result.stderr, /update conversion coverage/);

const driftedEvidence = canonicalEvidence();
delete driftedEvidence.conversion_tests.update;
result = await run({ evidence: driftedEvidence });
assert.notEqual(result.status, 0);
assert.match(result.stderr, /create\/update conversion-test evidence drift/);

console.log('Blog GraphQL target-only richtext guardrail tests passed.');
'''

EVIDENCE = {
    "schema_version": 3,
    "owner": "rustok-blog",
    "boundary": "graphql-post-richtext",
    "status": "implemented_source_verified_no_compile",
    "canonical_contract": {
        "write": "rustok_api::RichTextDocument",
        "read": "rustok_api::RichTextView",
        "plain_text": "server-derived",
        "resolver_conversion": "typed input.into() delegation with direct content mapping"
    },
    "scan_scope": "crates/rustok-blog/src/graphql/**/*.rs",
    "legacy_adapter_fields": [],
    "legacy_adapter_files": [],
    "conversion_owner": {
        "file": "crates/rustok-blog/src/graphql/types.rs",
        "scopes": [
            "impl From<CreatePostInput> for DomainCreatePostInput",
            "impl From<UpdatePostInput> for DomainUpdatePostInput"
        ],
        "reason": "target-only typed GraphQL-to-owner conversion"
    },
    "conversion_tests": {
        "create": {
            "file": "crates/rustok-blog/tests/graphql_create_post_input_conversion_test.rs",
            "markers": [
                "create_post_input_conversion_preserves_canonical_content",
                "RichTextDocument::single_paragraph",
                "let domain: DomainCreatePostInput = input.into();",
                "assert_eq!(domain.content, canonical)"
            ]
        },
        "update": {
            "file": "crates/rustok-blog/src/graphql/types.rs",
            "markers": [
                "update_post_input_conversion_preserves_canonical_content",
                "RichTextDocument::single_paragraph",
                "let domain: DomainUpdatePostInput = input.into();",
                "assert_eq!(domain.content, Some(canonical))"
            ]
        }
    },
    "guardrail": "scripts/verify/verify-blog-graphql-richtext-boundary.mjs",
    "guardrail_test": "scripts/verify/verify-blog-graphql-richtext-boundary.test.mjs",
    "exit_condition": "Source cutover complete; compile, migration, and runtime execution remain maintainer-owned."
}

(ROOT / 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs').write_text(VERIFIER, encoding='utf-8')
(ROOT / 'scripts/verify/verify-blog-graphql-richtext-boundary.test.mjs').write_text(SELF_TEST, encoding='utf-8')
(ROOT / 'crates/rustok-blog/contracts/evidence/blog-graphql-richtext-boundary.json').write_text(
    json.dumps(EVIDENCE, ensure_ascii=False, indent=2) + '\n',
    encoding='utf-8',
)

plan_path = ROOT / 'crates/rustok-blog/docs/implementation-plan.md'
plan = plan_path.read_text(encoding='utf-8')
old = '''paths are absent from production DTOs. The GraphQL mutation layer delegates
typed `input.into()` values and the recursive guardrail requires the removed
fields to remain absent.'''
new = '''paths are absent from production DTOs. The GraphQL mutation layer delegates
typed `input.into()` values through exact create/update owner conversions. The
recursive guardrail now evidence-locks both conversion scopes, requires direct
`content: input.content` mapping and both create/update regression markers, and
keeps the removed fields absent.'''
if old not in plan:
    raise SystemExit('implementation plan GraphQL paragraph anchor missing')
plan = plan.replace(old, new, 1)
old_slice = '''32. Extended the Blog admin canonical-richtext guardrail through the owner adapter
    itself: fixed Article frame profile, typed document round-trip, isolated
    no-referrer iframe, cleanup/dispose, evidence schema v3, and negative fixtures.

## Next results'''
new_slice = '''32. Extended the Blog admin canonical-richtext guardrail through the owner adapter
    itself: fixed Article frame profile, typed document round-trip, isolated
    no-referrer iframe, cleanup/dispose, evidence schema v3, and negative fixtures.
33. Bound the GraphQL target-only richtext verifier to evidence schema v3, exact
    create/update conversion scopes, direct canonical content mapping, and both
    conversion regression markers, with focused negative fixtures for each drift.

## Next results'''
if old_slice not in plan:
    raise SystemExit('implementation plan completed-slice anchor missing')
plan_path.write_text(plan.replace(old_slice, new_slice, 1), encoding='utf-8')
