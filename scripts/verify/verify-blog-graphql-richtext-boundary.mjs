#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import { join, relative, sep } from 'node:path';

const GRAPHQL_ROOT = 'crates/rustok-blog/src/graphql';
const TYPES_PATH = `${GRAPHQL_ROOT}/types.rs`;
const MUTATION_PATH = `${GRAPHQL_ROOT}/mutation.rs`;
const UPDATE_CONVERSION_START = 'impl From<UpdatePostInput> for DomainUpdatePostInput {';
const UPDATE_CONVERSION_END = '#[cfg(test)]\nmod tests {';

function normalizePath(path) {
  return path.split(sep).join('/');
}

async function collectRustFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...await collectRustFiles(path));
    } else if (entry.isFile() && entry.name.endsWith('.rs')) {
      files.push(normalizePath(path));
    }
  }

  return files;
}

function sourceRange(source, startMarker, endMarker, label) {
  const start = source.indexOf(startMarker);
  assert.notEqual(start, -1, `${label} is missing ${startMarker}`);
  const end = source.indexOf(endMarker, start + startMarker.length);
  assert.notEqual(end, -1, `${label} is missing ${endMarker}`);
  return { start, end, source: source.slice(start, end) };
}

function sourceBetween(source, startMarker, endMarker, label) {
  return sourceRange(source, startMarker, endMarker, label).source;
}

const graphqlFiles = await collectRustFiles(GRAPHQL_ROOT);
const sources = new Map(await Promise.all(
  graphqlFiles.map(async (path) => [path, await readFile(path, 'utf8')]),
));

const typesSource = sources.get(TYPES_PATH);
const mutationSource = sources.get(MUTATION_PATH);
assert.ok(typesSource, `${TYPES_PATH} must remain part of the Blog GraphQL boundary`);
assert.ok(mutationSource, `${MUTATION_PATH} must remain part of the Blog GraphQL boundary`);

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

const updateConversion = sourceRange(
  typesSource,
  UPDATE_CONVERSION_START,
  UPDATE_CONVERSION_END,
  TYPES_PATH,
);
assert.ok(
  updateConversion.source.includes('content: input.content'),
  'Blog GraphQL input conversion must forward RichTextDocument to the owner service',
);
assert.ok(
  !mutationSource.includes(UPDATE_CONVERSION_START),
  'Blog GraphQL mutation resolvers must not own UpdatePostInput transport conversion',
);

const resolverSources = new Map([
  [
    'create_post',
    sourceBetween(mutationSource, 'async fn create_post(', 'async fn update_post(', MUTATION_PATH),
  ],
  [
    'update_post',
    sourceBetween(mutationSource, 'async fn update_post(', 'async fn delete_post(', MUTATION_PATH),
  ],
]);
const resolverRichtextAccesses = [
  'input.body',
  'input.body_format',
  'input.content_json',
  'input.content',
];

for (const [resolver, source] of resolverSources) {
  assert.ok(
    source.includes('input.into()'),
    `Blog GraphQL ${resolver} must delegate transport conversion through input.into()`,
  );
  for (const access of resolverRichtextAccesses) {
    assert.ok(
      !source.includes(access),
      `Blog GraphQL ${resolver} must not wire richtext fields inside the async resolver: ${access}`,
    );
  }
}

const legacyFields = ['body', 'body_format', 'content_json'];
const legacyAdapterScopes = new Map([
  [TYPES_PATH, typesSource],
  [`${TYPES_PATH}::UpdatePostInput conversion`, updateConversion.source],
]);

for (const [scope, source] of legacyAdapterScopes) {
  for (const field of legacyFields) {
    assert.ok(
      source.includes(field),
      `${scope} no longer contains ${field}; update the evidence status and tighten this guardrail`,
    );
  }
}

const legacyLeakPatterns = [
  ['body', /\bpub\s+body\s*:/u],
  ['body', /\bbody\s*:\s*input\.body\b/u],
  ['body_format', /\bbody_format\b/u],
  ['content_json', /\bcontent_json\b/u],
];

for (const [path, source] of sources) {
  if (path === TYPES_PATH) {
    continue;
  }

  for (const [field, pattern] of legacyLeakPatterns) {
    assert.ok(
      !pattern.test(source),
      `Blog GraphQL legacy richtext field ${field} must stay confined to types.rs; found in ${normalizePath(relative('.', path))}`,
    );
  }
}

const forbiddenAliases = ['rt_json', 'markdown_body', 'raw_content_json'];
for (const [path, source] of sources) {
  for (const alias of forbiddenAliases) {
    assert.ok(
      !source.includes(alias),
      `Blog GraphQL must not introduce a new richtext alias ${alias}; found in ${normalizePath(relative('.', path))}`,
    );
  }
}

console.log('Blog GraphQL richtext boundary guardrail passed.');
