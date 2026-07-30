#!/usr/bin/env node
import assert from 'node:assert/strict';
import { copyFile, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
const verifier = resolve('scripts/verify/verify-blog-graphql-richtext-boundary.mjs');
async function run(types, mutation, createTest) {
  const root = await mkdtemp(join(tmpdir(), 'blog-gql-target-'));
  try {
    await mkdir(join(root, 'scripts/verify'), { recursive: true });
    await mkdir(join(root, 'crates/rustok-blog/src/graphql'), { recursive: true });
    await mkdir(join(root, 'crates/rustok-blog/tests'), { recursive: true });
    await copyFile(verifier, join(root, 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs'));
    await writeFile(join(root, 'crates/rustok-blog/src/graphql/types.rs'), types);
    await writeFile(join(root, 'crates/rustok-blog/src/graphql/mutation.rs'), mutation);
    await writeFile(join(root, 'crates/rustok-blog/tests/graphql_create_post_input_conversion_test.rs'), createTest);
    return spawnSync(process.execPath, ['scripts/verify/verify-blog-graphql-richtext-boundary.mjs'], { cwd: root, encoding: 'utf8' });
  } finally { await rm(root, { recursive: true, force: true }); }
}
const types = `
pub content: RichTextView,
pub content_plain_text: String,
pub content: RichTextDocument,
pub content: Option<RichTextDocument>,
impl From<CreatePostInput> for DomainCreatePostInput { fn from(input: CreatePostInput) -> Self { Self { content: input.content } } }
impl From<UpdatePostInput> for DomainUpdatePostInput { fn from(input: UpdatePostInput) -> Self { Self { content: input.content } } }
`;
const mutation = `
async fn create_post(input: CreatePostInput) { service.create_post(input.into()); }
async fn update_post(input: UpdatePostInput) { service.update_post(input.into()); }
async fn delete_post() {}
`;
const coverage = `fn create_post_input_conversion_preserves_canonical_content() {
let canonical = RichTextDocument::single_paragraph("canonical");
assert_eq!(domain.content, canonical);
}`;
let result = await run(types, mutation, coverage);
assert.equal(result.status, 0, result.stderr);
result = await run(types + '\npub body_format: String,', mutation, coverage);
assert.notEqual(result.status, 0);
result = await run(types, mutation.replace('input.into()', 'input.content'), coverage);
assert.notEqual(result.status, 0);
result = await run(types, mutation, coverage.replace('assert_eq!(domain.content, canonical)', ''));
assert.notEqual(result.status, 0);
console.log('Blog GraphQL target-only richtext guardrail tests passed.');
