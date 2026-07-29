#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const TYPES_PATH = 'crates/rustok-blog/src/graphql/types.rs';
const MUTATION_PATH = 'crates/rustok-blog/src/graphql/mutation.rs';

const [typesSource, mutationSource] = await Promise.all([
  readFile(TYPES_PATH, 'utf8'),
  readFile(MUTATION_PATH, 'utf8'),
]);

const canonicalTypeChecks = [
  'pub content: Option<RichTextView>',
  'pub content_plain_text: Option<String>',
  'pub content: Option<RichTextDocument>',
];

for (const needle of canonicalTypeChecks) {
  assert.ok(
    typesSource.includes(needle),
    `Blog GraphQL richtext boundary is missing canonical field: ${needle}`,
  );
}

assert.ok(
  mutationSource.includes('content: input.content'),
  'Blog GraphQL mutations must forward RichTextDocument to the owner service',
);

const legacyFields = ['body', 'body_format', 'content_json'];
const allowedLegacyFiles = new Map([
  [TYPES_PATH, typesSource],
  [MUTATION_PATH, mutationSource],
]);

for (const [path, source] of allowedLegacyFiles) {
  for (const field of legacyFields) {
    assert.ok(
      source.includes(field),
      `${path} no longer contains ${field}; update the evidence status and tighten this guardrail`,
    );
  }
}

const forbiddenAliases = ['rt_json', 'markdown_body', 'raw_content_json'];
for (const alias of forbiddenAliases) {
  assert.ok(
    !typesSource.includes(alias) && !mutationSource.includes(alias),
    `Blog GraphQL must not introduce a new richtext alias: ${alias}`,
  );
}

console.log('Blog GraphQL richtext boundary guardrail passed.');
