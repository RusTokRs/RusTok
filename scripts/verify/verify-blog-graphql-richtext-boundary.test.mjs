#!/usr/bin/env node

import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, copyFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = resolve('scripts/verify/verify-blog-graphql-richtext-boundary.mjs');

async function runFixture({ typesSource, mutationSource }) {
  const root = await mkdtemp(join(tmpdir(), 'blog-graphql-richtext-'));
  try {
    await mkdir(join(root, 'scripts/verify'), { recursive: true });
    await mkdir(join(root, 'crates/rustok-blog/src/graphql'), { recursive: true });
    await copyFile(verifier, join(root, 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs'));
    await writeFile(join(root, 'crates/rustok-blog/src/graphql/types.rs'), typesSource);
    await writeFile(join(root, 'crates/rustok-blog/src/graphql/mutation.rs'), mutationSource);
    return spawnSync(process.execPath, ['scripts/verify/verify-blog-graphql-richtext-boundary.mjs'], {
      cwd: root,
      encoding: 'utf8',
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

const canonicalTypes = `
pub content: Option<RichTextView>,
pub content_plain_text: Option<String>,
pub content: Option<RichTextDocument>,
pub body: Option<String>,
pub body_format: String,
pub content_json: Option<Value>,
`;
const canonicalMutation = `
body: input.body,
body_format: input.body_format,
content_json: input.content_json,
content: input.content,
`;

const passing = await runFixture({ typesSource: canonicalTypes, mutationSource: canonicalMutation });
assert.equal(passing.status, 0, passing.stderr);

const missingCanonical = await runFixture({
  typesSource: canonicalTypes.replace('pub content: Option<RichTextView>,', ''),
  mutationSource: canonicalMutation,
});
assert.notEqual(missingCanonical.status, 0);
assert.match(missingCanonical.stderr, /missing canonical field/);

const newAlias = await runFixture({
  typesSource: `${canonicalTypes}\npub markdown_body: Option<String>,`,
  mutationSource: canonicalMutation,
});
assert.notEqual(newAlias.status, 0);
assert.match(newAlias.stderr, /must not introduce a new richtext alias/);

const legacyRemoved = await runFixture({
  typesSource: canonicalTypes.replace('pub body_format: String,', ''),
  mutationSource: canonicalMutation,
});
assert.notEqual(legacyRemoved.status, 0);
assert.match(legacyRemoved.stderr, /update the evidence status and tighten this guardrail/);

console.log('Blog GraphQL richtext boundary guardrail tests passed.');
