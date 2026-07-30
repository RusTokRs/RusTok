#!/usr/bin/env node
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
