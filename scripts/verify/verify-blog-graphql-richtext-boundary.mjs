#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import { join, relative, sep } from 'node:path';

const GRAPHQL_ROOT = 'crates/rustok-blog/src/graphql';
const TYPES_PATH = `${GRAPHQL_ROOT}/types.rs`;
const MUTATION_PATH = `${GRAPHQL_ROOT}/mutation.rs`;
const UPDATE_CONVERSION_START = 'impl From<UpdatePostInput> for DomainUpdatePostInput {';
const UPDATE_CONVERSION_END = 'fn mutation_tenant_id(';
const TEST_MODULE_START = '#[cfg(test)]\nmod tests {';

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

function sourceRange(source, startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  assert.notEqual(start, -1, `Blog GraphQL mutation is missing ${startMarker}`);
  const end = source.indexOf(endMarker, start + startMarker.length);
  assert.notEqual(end, -1, `Blog GraphQL mutation is missing ${endMarker}`);
  return { start, end, source: source.slice(start, end) };
}

function sourceBetween(source, startMarker, endMarker) {
  return sourceRange(source, startMarker, endMarker).source;
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

const testModuleStart = mutationSource.indexOf(TEST_MODULE_START);
const mutationProductionSource = testModuleStart === -1
  ? mutationSource
  : mutationSource.slice(0, testModuleStart);
const updateConversion = sourceRange(
  mutationProductionSource,
  UPDATE_CONVERSION_START,
  UPDATE_CONVERSION_END,
);

assert.ok(
  updateConversion.source.includes('content: input.content'),
  'Blog GraphQL input conversion must forward RichTextDocument to the owner service',
);

const resolverSources = new Map([
  ['create_post', sourceBetween(mutationProductionSource, 'async fn create_post(', 'async fn update_post(')],
  ['update_post', sourceBetween(mutationProductionSource, 'async fn update_post(', 'async fn delete_post(')],
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
  [`${MUTATION_PATH}::UpdatePostInput conversion`, updateConversion.source],
]);

for (const [scope, source] of legacyAdapterScopes) {
  for (const field of legacyFields) {
    assert.ok(
      source.includes(field),
      `${scope} no longer contains ${field}; update the evidence status and tighten this guardrail`,
    );
  }
}

const mutationOutsideUpdateConversion = [
  mutationProductionSource.slice(0, updateConversion.start),
  mutationProductionSource.slice(updateConversion.end),
].join('');
const legacyScanSources = new Map(sources);
legacyScanSources.set(MUTATION_PATH, mutationOutsideUpdateConversion);

const legacyLeakPatterns = [
  ['body', /\bpub\s+body\s*:/u],
  ['body', /\bbody\s*:\s*input\.body\b/u],
  ['body_format', /\bbody_format\b/u],
  ['content_json', /\bcontent_json\b/u],
];

for (const [path, source] of legacyScanSources) {
  if (path === TYPES_PATH) {
    continue;
  }

  for (const [field, pattern] of legacyLeakPatterns) {
    assert.ok(
      !pattern.test(source),
      `Blog GraphQL production legacy richtext field ${field} must stay confined to types.rs or the isolated UpdatePostInput conversion; found in ${normalizePath(relative('.', path))}`,
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
