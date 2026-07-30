#!/usr/bin/env node
import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import { join, relative, sep } from 'node:path';

const ROOT = 'crates/rustok-blog/src/graphql';
const TYPES = `${ROOT}/types.rs`;
const MUTATION = `${ROOT}/mutation.rs`;
const CREATE_TEST = 'crates/rustok-blog/tests/graphql_create_post_input_conversion_test.rs';

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
const paths = await rustFiles(ROOT);
const sources = new Map(await Promise.all(paths.map(async path => [path, await readFile(path, 'utf8')])));
const types = sources.get(TYPES);
const mutation = sources.get(MUTATION);
const createTest = await readFile(CREATE_TEST, 'utf8');
for (const marker of [
  'pub content: RichTextView',
  'pub content_plain_text: String',
  'pub content: RichTextDocument',
  'pub content: Option<RichTextDocument>',
  'impl From<CreatePostInput> for DomainCreatePostInput',
  'impl From<UpdatePostInput> for DomainUpdatePostInput',
]) assert.ok(types.includes(marker), `missing target marker ${marker}`);
for (const [file, source] of sources) {
  for (const forbidden of ['pub body:', 'pub body_format:', 'pub content_json:', 'CONTENT_FORMAT_MARKDOWN', 'rt_json', 'markdown_body', 'raw_content_json']) {
    assert.ok(!source.includes(forbidden), `forbidden ${forbidden} in ${relative('.', file)}`);
  }
}
for (const source of [
  between(mutation, 'async fn create_post(', 'async fn update_post('),
  between(mutation, 'async fn update_post(', 'async fn delete_post('),
]) {
  assert.ok(source.includes('input.into()'), 'resolver must delegate through input.into()');
  assert.ok(!source.includes('input.content'), 'resolver must not map content manually');
}
for (const marker of [
  'create_post_input_conversion_preserves_canonical_content',
  'RichTextDocument::single_paragraph',
  'assert_eq!(domain.content, canonical)',
]) assert.ok(createTest.includes(marker), `missing create conversion coverage ${marker}`);
console.log('Blog GraphQL target-only richtext boundary guardrail passed.');
