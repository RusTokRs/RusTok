#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const boundary = read(
  'crates/rustok-commerce/src/graphql/safe_query/query_error_boundary.rs',
);
const facade = read('crates/rustok-commerce/src/graphql/safe_query.rs');
const sourceShim = read('crates/rustok-commerce/src/graphql/safe_query/source.rs');
const resolverSource = read('crates/rustok-commerce/src/graphql/query.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

const borrowedMapper = between(
  boundary,
  'impl QueryGraphqlMessage for &str {',
  'impl QueryGraphqlMessage for BoundaryError {',
  'borrowed query message mapper',
);
const typedGraphqlMapper = between(
  boundary,
  'impl From<Error> for BoundaryError {',
  'impl From<String> for BoundaryError {',
  'typed GraphQL mapper',
);
const publicRestoration = between(
  boundary,
  'impl From<BoundaryError> for Error {',
  '\n}',
  'public GraphQL restoration',
);

for (const [value, label] of [
  ['impl QueryGraphqlMessage for &str {', 'borrowed mapper definition'],
  ['let message_presence = text_presence_shape(self);', 'borrowed presence projection'],
  ['let message_len = self.len();', 'borrowed length projection'],
  ['let error = QueryDiagnosticError;', 'redacted diagnostic shadow'],
  ['tracing::error!(', 'borrowed diagnostic event'],
  ['error = ?error', 'redacted error field'],
  [
    'source_owner = "commerce_graphql_query.borrowed_message"',
    'borrowed diagnostic owner',
  ],
  ['error_kind = "borrowed_message"', 'borrowed diagnostic kind'],
  ['message_presence,', 'borrowed presence field'],
  ['message_len,', 'borrowed length field'],
  ['public_code = "COMMERCE_QUERY_OPERATION_FAILED"', 'stable public code field'],
  ['retryable = false', 'stable retryability field'],
  ['boundary = QUERY_ERROR_BOUNDARY', 'query boundary field'],
  [
    '"commerce GraphQL query borrowed error was redacted"',
    'static diagnostic message',
  ],
  ['BoundaryError::public(', 'stable public envelope construction'],
  [
    '"Commerce query could not be completed safely"',
    'stable public message',
  ],
  ['"COMMERCE_QUERY_OPERATION_FAILED"', 'stable public code'],
]) {
  requireText(borrowedMapper, value, label);
}

for (const value of [
  'BoundaryError::Graphql(Error::new(self))',
  'Error::new(self)',
  'error_message = %self',
  'message = %self',
  'error = %self',
  'message = ?self',
]) {
  forbidText(borrowedMapper, value, 'borrowed public or diagnostic bypass');
}

const projectionIndex = borrowedMapper.indexOf(
  'let message_presence = text_presence_shape(self);',
);
const shadowIndex = borrowedMapper.indexOf('let error = QueryDiagnosticError;');
const eventIndex = borrowedMapper.indexOf('tracing::error!(');
const publicIndex = borrowedMapper.indexOf('BoundaryError::public(');
if (
  !(
    projectionIndex >= 0 &&
    projectionIndex < shadowIndex &&
    shadowIndex < eventIndex &&
    eventIndex < publicIndex
  )
) {
  failures.push('borrowed message must project, shadow, diagnose, then map publicly');
}

for (const [pattern, expected, label] of [
  [/let message_presence = text_presence_shape\(self\);/g, 1, 'presence projection count'],
  [/let message_len = self\.len\(\);/g, 1, 'length projection count'],
  [/let error = QueryDiagnosticError;/g, 1, 'diagnostic shadow count'],
  [/error = \?error/g, 1, 'redacted error field count'],
  [/BoundaryError::public\(/g, 1, 'public envelope count'],
]) {
  const count = borrowedMapper.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['fn from(error: Error) -> Self', 'typed GraphQL conversion input'],
  ['Self::Graphql(error)', 'typed GraphQL pass-through'],
]) {
  requireText(typedGraphqlMapper, value, label);
}
for (const value of [
  'QueryDiagnosticError',
  'COMMERCE_QUERY_OPERATION_FAILED',
]) {
  forbidText(typedGraphqlMapper, value, 'typed GraphQL pass-through remapping');
}

for (const [content, value, label] of [
  [facade, 'mod query_error_boundary;', 'boundary module routing'],
  [facade, 'pub use source::CommerceQuery;', 'query export routing'],
  [sourceShim, 'pub type Error = super::super::query_error_boundary::BoundaryError;', 'shim error alias'],
  [sourceShim, 'include!("../query.rs");', 'unchanged resolver include'],
  [
    sourceShim,
    '<::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::permission_denied(message)',
    'permission extension construction',
  ],
  [publicRestoration, 'BoundaryError::Graphql(error) => error', 'typed restoration'],
  [publicRestoration, 'extensions.set("code", code)', 'public code extension'],
  [publicRestoration, 'extensions.set("retryable", retryable)', 'public retryability extension'],
]) {
  requireText(content, value, label);
}

const borrowedLiteralSites =
  resolverSource.match(/async_graphql::Error::new\(\s*"/g)?.length ?? 0;
if (borrowedLiteralSites !== 0) {
  failures.push(
    `expected no direct borrowed literal constructor sites in unchanged resolver source, found ${borrowedLiteralSites}`,
  );
}

const borrowedDefinitions =
  boundary.match(/impl QueryGraphqlMessage for &str \{/g)?.length ?? 0;
if (borrowedDefinitions !== 1) {
  failures.push(`borrowed mapper definition count: expected 1, found ${borrowedDefinitions}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL borrowed-message envelope verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL borrowed messages emit bounded diagnostics and cannot bypass the stable query envelope',
);
