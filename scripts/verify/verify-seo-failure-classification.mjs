#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const sliceBetween = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const errorSource = read('crates/rustok-seo/src/error.rs');
const libSource = read('crates/rustok-seo/src/lib.rs');

for (const [value, label] of [
  ['pub enum SeoFailureClass {', 'failure class enum'],
  ['Retryable,', 'retryable class'],
  ['Terminal,', 'terminal class'],
  ['Validation,', 'validation class'],
  ['Configuration,', 'configuration class'],
  ['pub struct SeoFailure {', 'failure envelope'],
  ['pub class: SeoFailureClass,', 'failure envelope class'],
  ['pub code: String,', 'failure stable code'],
  ['pub message: String,', 'failure message'],
  ['pub fn durable_message(&self) -> String', 'durable failure encoding'],
  ['pub fn parse_durable_message(value: &str) -> Option<Self>', 'durable failure parsing'],
  ['pub const fn failure_class(&self) -> SeoFailureClass', 'error classification method'],
  ['pub const fn stable_code(&self) -> &\'static str', 'stable error code method'],
  ['pub const fn is_retryable(&self) -> bool', 'retryability method'],
  ['pub fn failure(&self) -> SeoFailure', 'failure envelope conversion'],
]) {
  requireText(errorSource, value, label);
}

const classification = sliceBetween(
  errorSource,
  'pub const fn failure_class(&self) -> SeoFailureClass {',
  '\n    }',
  'failure classification mapping',
);
for (const [value, label] of [
  ['Self::Validation(_) => SeoFailureClass::Validation', 'validation mapping'],
  ['Self::Configuration(_) => SeoFailureClass::Configuration', 'configuration mapping'],
  ['Self::NotFound | Self::PermissionDenied => SeoFailureClass::Terminal', 'terminal mapping'],
  ['Self::Database(_) => SeoFailureClass::Retryable', 'retryable mapping'],
]) {
  requireText(classification, value, label);
}
forbidText(classification, '_ =>', 'implicit failure classification fallback');

const stableCodes = sliceBetween(
  errorSource,
  'pub const fn stable_code(&self) -> &\'static str {',
  '\n    }',
  'stable code mapping',
);
for (const value of [
  'Self::Validation(_) => "validation"',
  'Self::Configuration(_) => "configuration"',
  'Self::NotFound => "not_found"',
  'Self::PermissionDenied => "permission_denied"',
  'Self::Database(_) => "database"',
]) {
  requireText(stableCodes, value, 'stable code mapping');
}
forbidText(stableCodes, '_ =>', 'implicit stable code fallback');

for (const [value, label] of [
  ['pub use error::{SeoError, SeoFailure, SeoFailureClass, SeoResult};', 'public failure contract'],
  ['every_error_variant_has_an_explicit_failure_class', 'classification regression test'],
  ['durable_failure_message_round_trips_class_code_and_message', 'durable encoding regression test'],
]) {
  requireText(errorSource + libSource, value, label);
}

if (failures.length > 0) {
  console.error('SEO failure-classification verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ every SEO error maps explicitly to retryable, terminal, validation, or configuration with a stable durable envelope',
);
