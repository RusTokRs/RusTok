#!/usr/bin/env node

import assert from 'node:assert/strict';
import { copyFile, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = resolve('scripts/verify/verify-blog-graphql-richtext-boundary.mjs');

const canonicalCreateConversionTest = `
fn create_post_input_conversion_preserves_transport_fields() {
    let content = RichTextDocument::single_paragraph("canonical");
}

fn create_post_input_conversion_applies_legacy_defaults() {
    let format = rustok_core::CONTENT_FORMAT_MARKDOWN;
}
`;

async function runFixture({
  typesSource,
  mutationSource,
  createConversionTestSource = canonicalCreateConversionTest,
  extraFiles = {},
}) {
  const root = await mkdtemp(join(tmpdir(), 'blog-graphql-richtext-'));
  try {
    await mkdir(join(root, 'scripts/verify'), { recursive: true });
    await mkdir(join(root, 'crates/rustok-blog/src/graphql'), { recursive: true });
    await mkdir(join(root, 'crates/rustok-blog/tests'), { recursive: true });
    await copyFile(verifier, join(root, 'scripts/verify/verify-blog-graphql-richtext-boundary.mjs'));
    await writeFile(join(root, 'crates/rustok-blog/src/graphql/types.rs'), typesSource);
    await writeFile(join(root, 'crates/rustok-blog/src/graphql/mutation.rs'), mutationSource);
    await writeFile(
      join(root, 'crates/rustok-blog/tests/graphql_create_post_input_conversion_test.rs'),
      createConversionTestSource,
    );

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

const canonicalCreateConversion = `
impl From<CreatePostInput> for DomainCreatePostInput {
    fn from(input: CreatePostInput) -> Self {
        Self {
            body: input.body.unwrap_or_default(),
            body_format: input
                .body_format
                .unwrap_or_else(|| rustok_core::CONTENT_FORMAT_MARKDOWN.to_string()),
            content_json: input.content_json,
            content: input.content,
        }
    }
}
`;
const canonicalUpdateConversion = `
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
const canonicalTypes = `
pub content: Option<RichTextView>,
pub content_plain_text: Option<String>,
pub content: Option<RichTextDocument>,
pub body: Option<String>,
pub body_format: String,
pub content_json: Option<Value>,
${canonicalCreateConversion}
${canonicalUpdateConversion}
#[cfg(test)]
mod tests {}
`;
const canonicalMutation = `
async fn create_post(input: CreatePostInput) {
    let create_input: DomainCreatePostInput = input.into();
}

async fn update_post(input: UpdatePostInput) {
    let update_input: DomainUpdatePostInput = input.into();
}

async fn delete_post() {}

fn mutation_tenant_id() {}
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

const missingCreateConversionTest = await runFixture({
  typesSource: canonicalTypes,
  mutationSource: canonicalMutation,
  createConversionTestSource: '',
});
assert.notEqual(missingCreateConversionTest.status, 0);
assert.match(missingCreateConversionTest.stderr, /must cover CreatePostInput transport conversion/);

const manualCreateMapping = await runFixture({
  typesSource: canonicalTypes,
  mutationSource: canonicalMutation.replace(
    'let create_input: DomainCreatePostInput = input.into();',
    'let create_input = DomainCreatePostInput { body: input.body.unwrap_or_default(), body_format: input.body_format.unwrap_or_default(), content_json: input.content_json, content: input.content };',
  ),
});
assert.notEqual(manualCreateMapping.status, 0);
assert.match(manualCreateMapping.stderr, /must delegate transport conversion through input\.into\(\)/);

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

const mutationHelperLeak = await runFixture({
  typesSource: canonicalTypes,
  mutationSource: canonicalMutation.replace(
    'fn mutation_tenant_id() {}',
    'fn mutation_tenant_id() { let _legacy = input.content_json; }',
  ),
});
assert.notEqual(mutationHelperLeak.status, 0);
assert.match(mutationHelperLeak.stderr, /must stay confined to types\.rs/);

const mutationOwnsCreateConversion = await runFixture({
  typesSource: canonicalTypes,
  mutationSource: `${canonicalMutation}\n${canonicalCreateConversion}`,
});
assert.notEqual(mutationOwnsCreateConversion.status, 0);
assert.match(mutationOwnsCreateConversion.stderr, /must not own CreatePostInput transport conversion/);

const mutationOwnsUpdateConversion = await runFixture({
  typesSource: canonicalTypes,
  mutationSource: `${canonicalMutation}\n${canonicalUpdateConversion}`,
});
assert.notEqual(mutationOwnsUpdateConversion.status, 0);
assert.match(mutationOwnsUpdateConversion.stderr, /must not own UpdatePostInput transport conversion/);

const createConversionFieldRemoved = await runFixture({
  typesSource: canonicalTypes.replace('            content: input.content,\n', ''),
  mutationSource: canonicalMutation,
});
assert.notEqual(createConversionFieldRemoved.status, 0);
assert.match(createConversionFieldRemoved.stderr, /CreatePostInput conversion no longer contains/);

const updateConversionFieldRemoved = await runFixture({
  typesSource: canonicalTypes.replace('            body_format: input.body_format,\n', ''),
  mutationSource: canonicalMutation,
});
assert.notEqual(updateConversionFieldRemoved.status, 0);
assert.match(updateConversionFieldRemoved.stderr, /UpdatePostInput conversion no longer contains/);

const createCanonicalCoverageRemoved = await runFixture({
  typesSource: canonicalTypes,
  mutationSource: canonicalMutation,
  createConversionTestSource: canonicalCreateConversionTest.replace(
    'RichTextDocument::single_paragraph',
    'legacy_document',
  ),
});
assert.notEqual(createCanonicalCoverageRemoved.status, 0);
assert.match(createCanonicalCoverageRemoved.stderr, /missing create conversion coverage/);

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
assert.match(legacyLeak.stderr, /must stay confined to types\.rs/);

const legacyDeclarationRemoved = await runFixture({
  typesSource: canonicalTypes.replace('pub body_format: String,', ''),
  mutationSource: canonicalMutation,
});
assert.notEqual(legacyDeclarationRemoved.status, 0);
assert.match(legacyDeclarationRemoved.stderr, /update the evidence status and tighten this guardrail/);

console.log('Blog GraphQL richtext boundary guardrail tests passed.');
