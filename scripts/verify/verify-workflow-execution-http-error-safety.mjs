#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const controller = read('crates/rustok-workflow/src/controllers/executions.rs');
const ownerError = read('crates/rustok-workflow/src/error.rs');
const ownerService = read('crates/rustok-workflow/src/services/workflow_service.rs');
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

const mapper = between(
  controller,
  'fn map_workflow_execution_error(',
  'pub async fn list_executions(',
  'workflow execution error mapper',
);

for (const [value, label] of [
  ['WorkflowError::NotFound(_)', 'workflow not-found variant'],
  ['WorkflowError::StepNotFound(_)', 'unexpected step variant'],
  ['WorkflowError::ExecutionNotFound(_)', 'execution not-found variant'],
  ['WorkflowError::NotActive(_)', 'state conflict variant'],
  ['WorkflowError::StepFailed(_)', 'step failure variant'],
  ['WorkflowError::UnknownStepType(_)', 'unknown step type variant'],
  ['WorkflowError::InvalidTriggerConfig(_)', 'invalid trigger variant'],
  ['WorkflowError::InvalidStepConfig(_)', 'invalid step config variant'],
  ['WorkflowError::Database(_)', 'database variant'],
  ['WorkflowError::Serialization(_)', 'serialization variant'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::BAD_REQUEST', 'validation status'],
  ['StatusCode::CONFLICT', 'state conflict status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'storage unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed status'],
  ['"workflow_not_found"', 'workflow not-found code'],
  ['"workflow_execution_not_found"', 'execution not-found code'],
  ['"workflow_execution_invalid"', 'execution invalid code'],
  ['"workflow_state_conflict"', 'state conflict code'],
  ['"workflow_storage_unavailable"', 'storage unavailable code'],
  ['"workflow_execution_failed"', 'execution failure code'],
  ['owner = "rustok_workflow.workflow_service"', 'owner logging'],
  ['operation,', 'operation logging'],
  ['tenant_id = %tenant_id', 'tenant logging'],
  ['workflow_id = ?workflow_id', 'workflow logging'],
  ['execution_id = ?execution_id', 'execution logging'],
  ['error_kind,', 'error kind logging'],
  ['public_code = code', 'public code logging'],
  ['status = %status', 'status logging'],
  ['boundary = "workflow_execution_http"', 'boundary logging'],
  ['HttpError::new(status, code, message)', 'static public envelope'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  ['"list_executions",', 'list operation label'],
  ['Some(workflow_id),', 'list workflow context'],
  ['"get_execution",', 'get operation label'],
  ['Some(execution_id),', 'get execution context'],
  ['Permission::WORKFLOW_EXECUTIONS_LIST', 'list permission'],
  ['Permission::WORKFLOW_EXECUTIONS_READ', 'read permission'],
  ['"workflow_permission_denied"', 'permission code'],
  ['Ok(Json(executions))', 'list success payload'],
  ['Ok(Json(execution))', 'get success payload'],
]) requireText(controller, value, label);

for (const value of [
  'err.to_string()',
  'other.to_string()',
  'error.to_string()',
  'HttpError::bad_request("workflow_operation_failed"',
]) forbidText(controller, value, 'unsafe workflow execution public conversion');

for (const [value, label] of [
  ['pub enum WorkflowError {', 'workflow owner enum'],
  ['NotFound(Uuid)', 'owner workflow not-found variant'],
  ['StepNotFound(Uuid)', 'owner step not-found variant'],
  ['ExecutionNotFound(Uuid)', 'owner execution not-found variant'],
  ['NotActive(String)', 'owner not-active variant'],
  ['StepFailed(String)', 'owner step-failed variant'],
  ['UnknownStepType(String)', 'owner unknown-step variant'],
  ['InvalidTriggerConfig(String)', 'owner invalid-trigger variant'],
  ['InvalidStepConfig(String)', 'owner invalid-step variant'],
  ['Database(#[from] sea_orm::DbErr)', 'owner database variant'],
  ['Serialization(#[from] serde_json::Error)', 'owner serialization variant'],
]) requireText(ownerError, value, label);

const ownerExecutionQueries = between(
  ownerService,
  'pub async fn list_executions(',
  '// ── Webhook trigger',
  'owner execution query operations',
);
for (const [value, label] of [
  ['pub async fn list_executions(', 'owner list operation'],
  ['pub async fn get_execution(', 'owner get operation'],
  ['.filter(workflow_execution::Column::TenantId.eq(tenant_id))', 'tenant execution guard'],
  ['.ok_or(WorkflowError::ExecutionNotFound(execution_id))?', 'owner execution guard'],
]) requireText(ownerExecutionQueries, value, label);

const mapperUses = controller.match(/map_workflow_execution_error\(/g) ?? [];
if (mapperUses.length !== 3) {
  failures.push(`expected mapper definition plus two uses, found ${mapperUses.length}`);
}

if (failures.length > 0) {
  console.error('Workflow execution HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Workflow execution owner errors use typed safe HTTP envelopes');
