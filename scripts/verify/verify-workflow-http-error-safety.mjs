#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const controller = read('crates/rustok-workflow/src/controllers/workflows.rs');
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
  'fn map_workflow_error(',
  'pub async fn list(',
  'workflow error mapper',
);

for (const [value, label] of [
  ['WorkflowError::NotFound(_)', 'workflow not-found variant'],
  ['WorkflowError::StepNotFound(_)', 'unexpected step variant'],
  ['WorkflowError::ExecutionNotFound(_)', 'unexpected execution variant'],
  ['WorkflowError::NotActive(_)', 'state conflict variant'],
  ['WorkflowError::StepFailed(_)', 'execution failure variant'],
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
  ['"workflow_invalid"', 'workflow invalid code'],
  ['"workflow_state_conflict"', 'state conflict code'],
  ['"workflow_storage_unavailable"', 'storage unavailable code'],
  ['"workflow_execution_failed"', 'execution failure code'],
  ['"workflow_operation_failed"', 'operation failure code'],
  ['owner = "rustok_workflow.workflow_service"', 'owner logging'],
  ['operation,', 'operation logging'],
  ['tenant_id = %tenant_id', 'tenant logging'],
  ['workflow_id = ?workflow_id', 'workflow logging'],
  ['error_kind,', 'error kind logging'],
  ['public_code = code', 'public code logging'],
  ['status = %status', 'status logging'],
  ['boundary = "workflow_http"', 'boundary logging'],
  ['HttpError::new(status, code, message)', 'static public envelope'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  ['map_workflow_error(error, "list", tenant.id, None)', 'list mapper'],
  ['map_workflow_error(error, "get", tenant.id, Some(id))', 'get mapper'],
  ['map_workflow_error(error, "create", tenant.id, None)', 'create mapper'],
  ['map_workflow_error(error, "update", tenant.id, Some(id))', 'update mapper'],
  ['map_workflow_error(error, "delete", tenant.id, Some(id))', 'delete mapper'],
  ['map_workflow_error(error, "activate", tenant.id, Some(id))', 'activate mapper'],
  ['map_workflow_error(error, "pause", tenant.id, Some(id))', 'pause mapper'],
  ['map_workflow_error(error, "trigger_manual", tenant.id, Some(id))', 'manual trigger mapper'],
]) requireText(controller, value, label);

for (const [value, label] of [
  ['Permission::WORKFLOWS_LIST', 'list permission'],
  ['Permission::WORKFLOWS_READ', 'read permission'],
  ['Permission::WORKFLOWS_CREATE', 'create permission'],
  ['Permission::WORKFLOWS_UPDATE', 'update permission'],
  ['Permission::WORKFLOWS_DELETE', 'delete permission'],
  ['Permission::WORKFLOWS_EXECUTE', 'execute permission'],
  ['"workflow_permission_denied"', 'permission code'],
  ['serde_json::json!({ "id": id })', 'create success payload'],
  ['serde_json::json!({ "ok": true })', 'mutation success payload'],
  ['serde_json::json!({ "execution_id": execution_id })', 'trigger success payload'],
]) requireText(controller, value, label);

for (const value of [
  'err.to_string()',
  'error.to_string()',
  'HttpError::bad_request("workflow_operation_failed"',
]) forbidText(controller, value, 'unsafe workflow public conversion');

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

for (const [value, label] of [
  ['pub async fn create(', 'owner create operation'],
  ['pub async fn get(', 'owner get operation'],
  ['pub async fn list(', 'owner list operation'],
  ['pub async fn update(', 'owner update operation'],
  ['pub async fn delete(', 'owner delete operation'],
  ['pub async fn trigger_manual(', 'owner manual trigger operation'],
  ['.ok_or(WorkflowError::NotFound(id))?', 'owner workflow guard'],
  ['.ok_or(WorkflowError::NotFound(workflow_id))?', 'owner trigger guard'],
  ['return Err(WorkflowError::NotActive(', 'owner active-state guard'],
]) requireText(ownerService, value, label);

const mapperUses = controller.match(/map_workflow_error\(/g) ?? [];
if (mapperUses.length !== 9) {
  failures.push(`expected mapper definition plus eight uses, found ${mapperUses.length}`);
}

if (failures.length > 0) {
  console.error('Workflow HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Workflow owner errors use typed safe HTTP envelopes');
