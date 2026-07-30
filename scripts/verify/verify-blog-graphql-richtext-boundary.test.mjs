#!/usr/bin/env node

import assert from 'node:assert/strict';
import { copyFile, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = resolve('scripts/verify/verify-blog-graphql-richtext-boundary.mjs');

async function runFixture({ typesSource, mutationSource, extraFiles = {} }) {
  const root = await mkdtemp(join(tmpdir(), 'blog-graphql-richtext-'));
  try {
    await mkdir(join(root, 'scripts/verify'), { recursive: true });
    await mkdir(join(root, 'crates/rustok-blog/src/graphql'), { recursive: true });
    await copyFile(verifier, join(root, 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs'));
    await writeFile(join(root, 'crates/rustok-blog/src/graphql/types.rs'), typesSource);
    await writeFile(join(root, 'crates/rustok-blog/src/graphql/mutation.rs'), mutationSource);

    for (const [path, source] of Object.entries(extraFiles)) {
      const target = join(root, path);
      await mkdir(dirname(target), { recursive: true });
      await writeFile(target, source);
    }

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
async fn create_post(input: CreatePostInput) {
    let create_input: DomainCreatePostInput = input.into();
}

async fn update_post(input: UpdatePostInput) {
    let update_input: DomainUpdatePostInput = input.into();
}

async fn delete_post() {}

impl From<UpdatePostInput> for DomainUpdatePostInput {
    fn from(input: UpdatePostInput) -> Self {
        Self {
            body: input.body,
            body_format: input.body_format,
            content_json: input.content_json,
            content: input.content,
        }
    }
}
`;

const passing = await runFixture({
  typesSource: canonicalTypes,
  mutationSource: canonicalMutation,
  extraFiles: {
    'crates/rustok-blog/src/graphql/query.rs': 'pub async fn post() {}',
  },
});
assert.equal(passing.status, 0, passing.stderr);

const missingCanonical = await runFixture({
  typesSource: canonicalTypes.replace('pub content: Option<RichTextView>,', ''),
  mutationSource: canonicalMutation,
});
assert.notEqual(missingCanonical.status, 0);
assert.match(missingCanonical.stderr, /missing canonical field/);

const manualUpdateMapping = await runFixture({
  typesSource: canonicalTypes,
  mutationSource: canonicalMutation.replace(
    'let update_input: DomainUpdatePostInput = input.into();',
    'let update_input = DomainUpdatePostInput { body: input.body, body_format: input.body_format, content_json: input.content_json, content: input.content };',
  ),
});
assert.notEqual(manualUpdateMapping.status, 0);
assert.match(manualUpdateMapping.stderr, /must delegate transport conversion through input\.into\(\)/);

const resolverFieldAccess = await runFixture({
  typesSource: canonicalTypes,
  mutationSource: canonicalMutation.replace(
    'let update_input: DomainUpdatePostInput = input.into();',
    'let _legacy = input.content_json.as_ref();\n    let update_input: DomainUpdatePostInput = input.into();',
  ),
});
assert.notEqual(resolverFieldAccess.status, 0);
assert.match(resolverFieldAccess.stderr, /must not wire richtext fields inside the async resolver/);

const newAlias = await runFixture({
  typesSource: canonicalTypes,
  mutationSource: canonicalMutation,
  extraFiles: {
    'crates/rustok-blog/src/graphql/query.rs': 'pub markdown_body: Option<String>,',
  },
});
assert.notEqual(newAlias.status, 0);
assert.match(newAlias.stderr, /must not introduce a new richtext alias/);

const legacyLeak = await runFixture({
  typesSource: canonicalTypes,
  mutationSource: canonicalMutation,
  extraFiles: {
    'crates/rustok-blog/src/graphql/nested/query.rs': 'pub content_json: Option<Value>,',
  },
});
assert.notEqual(legacyLeak.status, 0);
assert.match(legacyLeak.stderr, /must stay confined to the adapter allowlist/);

const legacyRemoved = await runFixture({
  typesSource: canonicalTypes.replace('pub body_format: String,', ''),
  mutationSource: canonicalMutation,
});
assert.notEqual(legacyRemoved.status, 0);
assert.match(legacyRemoved.stderr, /update the evidence status and tighten this guardrail/);

console.log('Blog GraphQL richtext boundary guardrail tests passed.');
