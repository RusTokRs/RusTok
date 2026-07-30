#!/usr/bin/env node
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
