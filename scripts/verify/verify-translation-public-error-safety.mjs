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

const paths = {
  mapper: 'crates/rustok-translation/src/public_error.rs',
  error: 'crates/rustok-translation/src/error.rs',
  root: 'crates/rustok-translation/src/lib.rs',
  graphql: 'crates/rustok-translation/src/graphql/context.rs',
  native:
    'crates/rustok-translation/admin/src/transport/native_server_adapter.rs',
  evidence:
    'crates/rustok-translation/contracts/evidence/translation-public-error-safety-source.json',
  review:
    'crates/rustok-translation/contracts/evidence/translation-public-error-safety-source-review.json',
  document: 'crates/rustok-translation/docs/translation-public-error-safety.md',
};

const mapper = read(paths.mapper);
const errorSource = read(paths.error);
const rootSource = read(paths.root);
const graphql = read(paths.graphql);
const native = read(paths.native);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const document = read(paths.document);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (source, value) => source.split(value).length - 1;

for (const marker of [
  'pub enum TranslationPublicErrorKind {',
  'Forbidden,',
  'NotFound,',
  'BadInput,',
  'Internal,',
  'pub struct TranslationPublicError {',
  'pub kind: TranslationPublicErrorKind',
  'pub message: String',
  "pub code: &'static str",
  'pub retryable: bool',
  'pub correlation_id: Uuid',
  'impl std::fmt::Display for TranslationPublicError',
  '"{} (code: {}; reference: {})"',
  'pub fn map_translation_public_error(',
]) requireText(mapper, marker, `${paths.mapper}: public contract`);

requireText(
  rootSource,
  'TranslationPublicError, TranslationPublicErrorKind, map_translation_public_error,',
  `${paths.root}: shared export`,
);

const ownerVariantCount = (
  errorSource.match(/^\s{4}[A-Z][A-Za-z0-9_]*(?:\s*\{|\s*\(|,)/gm) ?? []
).length;
if (ownerVariantCount !== 89) {
  failures.push(`${paths.error}: expected 89 TranslationError variants, found ${ownerVariantCount}`);
}

for (const marker of [
  'let (kind, message, code, retryable, error_class) = match error {',
  'TranslationError::Forbidden =>',
  'TranslationPublicErrorKind::Forbidden',
  '"Translation permission denied".to_string()',
  '"TRANSLATION_PERMISSION_DENIED"',
  '"forbidden"',
  'TranslationError::JobNotFound',
  '| TranslationError::ItemNotFound',
  '| TranslationError::WorkflowNoteNotFound',
  '| TranslationError::InterchangeArtifactNotFound',
  '| TranslationError::InterchangeArtifactExpired',
  '| TranslationError::ProposalNotFound',
  '| TranslationError::JobProgressNotFound',
  '| TranslationError::GlossaryNotFound',
  '| TranslationError::MemoryEntryNotFound',
  '| TranslationError::MachineOperationNotFound',
  'TranslationPublicErrorKind::NotFound',
  '"Translation resource was not found".to_string()',
  '"TRANSLATION_RESOURCE_NOT_FOUND"',
  '"not_found"',
  'TranslationError::InvalidRequest(_)',
  '| TranslationError::WorkflowRevisionConflict',
  '| TranslationError::JobNotWritable(_)',
  '| TranslationError::TranslationPolicyConflict { .. }',
  '| TranslationError::RequiredTargetLocaleDisabled(_)',
  '| TranslationError::DisabledJobLocale { .. }',
  '| TranslationError::GlossaryRevisionConflict { .. }',
  '| TranslationError::GlossaryRevisionUnavailable { .. }',
  '| TranslationError::MemoryRevisionConflict { .. }',
  '| TranslationError::MachineOperationTerminal(_)',
  '| TranslationError::InterchangeArtifactNotReady',
  '| TranslationError::InterchangeArtifactAlreadyProcessed',
  '| TranslationError::MemoryRetentionConflict(_)',
  'TranslationPublicErrorKind::BadInput',
  '"Translation request is invalid".to_string()',
  '"TRANSLATION_REQUEST_INVALID"',
  '"bad_input"',
  'TranslationError::Provider {',
  'retryable: true, ..',
  '| TranslationError::InterchangeArtifactInProgress',
  '| TranslationError::MachineRecoveryResultUnavailable',
  '| TranslationError::Database(_)',
  '"Translation service is temporarily unavailable".to_string()',
  '"TRANSLATION_TEMPORARILY_UNAVAILABLE"',
  '"temporarily_unavailable"',
  '"Translation operation could not be completed".to_string()',
  '"TRANSLATION_OPERATION_FAILED"',
  '"internal"',
  'let correlation_id = Uuid::new_v4();',
  'tracing::error!(',
  'error_class,',
  'operation,',
  'boundary,',
  'public_code = code',
  'retryable,',
  '%correlation_id',
  'TranslationPublicError {',
  'kind,',
  'message,',
  'code,',
  'correlation_id,',
]) requireText(mapper, marker, `${paths.mapper}: stable shared mapping`);

for (const forbidden of [
  'error.to_string()',
  'error = ?error',
  'error = %error',
  'provider_code =',
  'provider_message =',
  'locale = %',
  'locale = ?',
  'revision =',
  'reason = %',
  'reason = ?',
  'state = %',
  'state = ?',
]) forbidText(mapper, forbidden, `${paths.mapper}: owner payload exposure`);

if (countText(mapper, '"Translation resource was not found".to_string()') !== 1) {
  failures.push(`${paths.mapper}: not-found envelope must be unique`);
}
if (countText(mapper, '"Translation request is invalid".to_string()') !== 1) {
  failures.push(`${paths.mapper}: bad-input envelope must be unique`);
}

for (const marker of [
  'pub(crate) fn translation_error(error: crate::TranslationError) -> Error',
  'crate::map_translation_public_error(',
  '"graphql_operation"',
  '"translation_graphql"',
  'crate::TranslationPublicErrorKind::Forbidden =>',
  'crate::TranslationPublicErrorKind::NotFound =>',
  'crate::TranslationPublicErrorKind::BadInput =>',
  'crate::TranslationPublicErrorKind::Internal =>',
]) requireText(graphql, marker, `${paths.graphql}: preserved shared consumer`);

for (const marker of [
  'fn public_error(error: rustok_translation::TranslationError) -> ServerFnError',
  'rustok_translation::map_translation_public_error(',
  '"native_operation"',
  '"translation_admin_native"',
  '.to_string()',
]) requireText(native, marker, `${paths.native}: preserved shared consumer`);

for (const [key, expected] of Object.entries({
  public_kind_changed: false,
  public_code_changed: false,
  public_retryability_changed: false,
  forbidden_message_changed: false,
  not_found_dynamic_payload_removed: true,
  bad_input_dynamic_payload_removed: true,
  internal_messages_changed: false,
  complete_translation_error_logged: false,
  provider_error_payload_logged: false,
  database_error_payload_logged: false,
  workflow_state_payload_logged: false,
  locale_revision_reason_payload_logged: false,
  static_error_class_logged: true,
  operation_logged: true,
  boundary_logged: true,
  public_code_logged: true,
  retryability_logged: true,
  correlation_logged: true,
  correlation_generation_changed: false,
  correlation_rendering_changed: false,
  graphql_consumer_changed: false,
  native_consumer_changed: false,
  shared_export_changed: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const key of [
  'tests_run',
  'verifiers_run',
  'cargo_run',
  'format_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'graphql_runtime_proven',
  'native_runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
}

for (const [key, expected] of Object.entries({
  public_structs_preserved: true,
  public_kind_classification_preserved: true,
  public_codes_preserved: true,
  public_retryability_preserved: true,
  correlation_envelope_preserved: true,
  forbidden_message_preserved: true,
  not_found_payload_redacted: true,
  bad_input_payload_redacted: true,
  internal_messages_preserved: true,
  complete_translation_error_logging_removed: true,
  provider_database_payload_removed: true,
  workflow_locale_revision_reason_payload_removed: true,
  static_error_class_retained: true,
  graphql_consumer_preserved: true,
  native_consumer_preserved: true,
  shared_export_preserved: true,
  broad_ecommerce_cleanup_remains_open: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  '`map_translation_public_error`',
  'GraphQL and native admin consumers',
  '`Translation resource was not found`',
  '`Translation request is invalid`',
  'The broader ecommerce mapper cleanup remains open.',
]) requireText(document, marker, `${paths.document}: truthful source scope`);

if (failures.length > 0) {
  console.error('Translation shared public-error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Translation shared public errors use static public envelopes and bounded diagnostics; execution evidence remains open',
);
