import { graphqlRequest, type GqlOpts } from '@/lib/graphql';
export type { GqlOpts };

// ---------- Types ----------

type WorkflowStatus = 'DRAFT' | 'ACTIVE' | 'PAUSED' | 'ARCHIVED';
type StepType =
  | 'ACTION'
  | 'CONDITION'
  | 'DELAY'
  | 'ALLOY_SCRIPT'
  | 'EMIT_EVENT'
  | 'HTTP'
  | 'NOTIFY'
  | 'TRANSFORM';
type OnError = 'STOP' | 'SKIP' | 'RETRY';
type ExecutionStatus = 'RUNNING' | 'COMPLETED' | 'FAILED' | 'TIMED_OUT';
type StepExecutionStatus =
  'PENDING' | 'RUNNING' | 'COMPLETED' | 'FAILED' | 'SKIPPED';

export interface WorkflowSummary {
  id: string;
  tenantId: string;
  name: string;
  status: WorkflowStatus;
  webhookSlug: string | null;
  failureCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface WorkflowStep {
  id: string;
  workflowId: string;
  position: number;
  stepType: StepType;
  config: Record<string, unknown>;
  onError: OnError;
  timeoutMs: number | null;
}

export interface WorkflowResponse {
  id: string;
  tenantId: string;
  name: string;
  description: string | null;
  status: WorkflowStatus;
  triggerConfig: Record<string, unknown>;
  webhookSlug: string | null;
  createdBy: string | null;
  createdAt: string;
  updatedAt: string;
  failureCount: number;
  autoDisabledAt: string | null;
  steps: WorkflowStep[];
}

interface StepExecution {
  id: string;
  executionId: string;
  stepId: string;
  status: StepExecutionStatus;
  input: Record<string, unknown>;
  output: Record<string, unknown>;
  error: string | null;
  startedAt: string;
  completedAt: string | null;
}

export interface WorkflowExecution {
  id: string;
  workflowId: string;
  tenantId: string;
  triggerEventId: string | null;
  status: ExecutionStatus;
  context: Record<string, unknown>;
  error: string | null;
  startedAt: string;
  completedAt: string | null;
  stepExecutions: StepExecution[];
}

export interface CreateWorkflowInput {
  name: string;
  description?: string;
  triggerConfig: Record<string, unknown>;
  webhookSlug?: string;
}

export interface UpdateWorkflowInput {
  name?: string;
  description?: string;
  status?: WorkflowStatus;
  triggerConfig?: Record<string, unknown>;
  webhookSlug?: string;
}

// ---------- Phase 4 Types ----------

export interface WorkflowTemplate {
  id: string;
  name: string;
  description: string;
  category: string;
}

export interface WorkflowVersionSummary {
  version: number;
  createdAt: string;
  createdBy: string | null;
}

export interface CreateStepInput {
  position: number;
  stepType: StepType;
  config: Record<string, unknown>;
  onError: OnError;
  timeoutMs?: number;
}

interface UpdateStepInput {
  position?: number;
  stepType?: StepType;
  config?: Record<string, unknown>;
  onError?: OnError;
  timeoutMs?: number;
}

// ---------- GraphQL operations (tenantId resolved server-side from headers) ----------

export const WORKFLOWS_QUERY = `
query Workflows {
  workflows {
    id
    tenantId
    name
    status
    webhookSlug
    failureCount
    createdAt
    updatedAt
  }
}`;

export const WORKFLOW_QUERY = `
query Workflow($id: UUID!) {
  workflow(id: $id) {
    id
    tenantId
    name
    description
    status
    triggerConfig
    webhookSlug
    createdBy
    createdAt
    updatedAt
    failureCount
    autoDisabledAt
    steps {
      id
      workflowId
      position
      stepType
      config
      onError
      timeoutMs
    }
  }
}`;

export const WORKFLOW_EXECUTIONS_QUERY = `
query WorkflowExecutions($workflowId: UUID!) {
  workflowExecutions(workflowId: $workflowId) {
    id
    workflowId
    tenantId
    status
    error
    startedAt
    completedAt
    stepExecutions {
      id
      stepId
      status
      error
      startedAt
      completedAt
    }
  }
}`;

export const CREATE_WORKFLOW_MUTATION = `
mutation CreateWorkflow($input: GqlCreateWorkflowInput!) {
  createWorkflow(input: $input)
}`;

export const UPDATE_WORKFLOW_MUTATION = `
mutation UpdateWorkflow($id: UUID!, $input: GqlUpdateWorkflowInput!) {
  updateWorkflow(id: $id, input: $input)
}`;

export const DELETE_WORKFLOW_MUTATION = `
mutation DeleteWorkflow($id: UUID!) {
  deleteWorkflow(id: $id)
}`;

export const ACTIVATE_WORKFLOW_MUTATION = `
mutation ActivateWorkflow($id: UUID!) {
  activateWorkflow(id: $id)
}`;

export const PAUSE_WORKFLOW_MUTATION = `
mutation PauseWorkflow($id: UUID!) {
  pauseWorkflow(id: $id)
}`;

export const TRIGGER_WORKFLOW_MUTATION = `
mutation TriggerWorkflow($id: UUID!, $payload: JSON, $force: Boolean) {
  triggerWorkflow(id: $id, payload: $payload, force: $force)
}`;

export const ADD_STEP_MUTATION = `
mutation AddWorkflowStep($workflowId: UUID!, $input: GqlCreateStepInput!) {
  addWorkflowStep(workflowId: $workflowId, input: $input)
}`;

export const UPDATE_STEP_MUTATION = `
mutation UpdateWorkflowStep($workflowId: UUID!, $stepId: UUID!, $input: GqlUpdateStepInput!) {
  updateWorkflowStep(workflowId: $workflowId, stepId: $stepId, input: $input)
}`;

export const DELETE_STEP_MUTATION = `
mutation DeleteWorkflowStep($workflowId: UUID!, $stepId: UUID!) {
  deleteWorkflowStep(workflowId: $workflowId, stepId: $stepId)
}`;

// ---------- API functions ----------

export async function listWorkflows(
  opts: GqlOpts = {}
): Promise<WorkflowSummary[]> {
  const data = await graphqlRequest<
    Record<string, never>,
    { workflows: WorkflowSummary[] }
  >(WORKFLOWS_QUERY, {}, opts.token, opts.tenantSlug);
  return data.workflows;
}

export async function getWorkflow(
  id: string,
  opts: GqlOpts = {}
): Promise<WorkflowResponse | null> {
  const cleanId = id?.trim();
  if (!cleanId) {
    throw new Error('Workflow ID is required');
  }
  const data = await graphqlRequest<
    { id: string },
    { workflow: WorkflowResponse | null }
  >(WORKFLOW_QUERY, { id: cleanId }, opts.token, opts.tenantSlug);
  return data.workflow;
}

export async function listWorkflowExecutions(
  workflowId: string,
  opts: GqlOpts = {}
): Promise<WorkflowExecution[]> {
  const cleanId = workflowId?.trim();
  if (!cleanId) {
    throw new Error('Workflow ID is required');
  }
  const data = await graphqlRequest<
    { workflowId: string },
    { workflowExecutions: WorkflowExecution[] }
  >(WORKFLOW_EXECUTIONS_QUERY, { workflowId: cleanId }, opts.token, opts.tenantSlug);
  return data.workflowExecutions;
}

export async function createWorkflow(
  input: CreateWorkflowInput,
  opts: GqlOpts = {}
): Promise<string> {
  const cleanName = input.name?.trim();
  if (!cleanName) {
    throw new Error('Workflow name is required');
  }
  const payload: CreateWorkflowInput = {
    ...input,
    name: cleanName
  };
  const data = await graphqlRequest<
    { input: CreateWorkflowInput },
    { createWorkflow: string }
  >(CREATE_WORKFLOW_MUTATION, { input: payload }, opts.token, opts.tenantSlug);
  return data.createWorkflow;
}

export async function updateWorkflow(
  id: string,
  input: UpdateWorkflowInput,
  opts: GqlOpts = {}
): Promise<void> {
  const cleanId = id?.trim();
  if (!cleanId) {
    throw new Error('Workflow ID is required');
  }
  await graphqlRequest<
    { id: string; input: UpdateWorkflowInput },
    { updateWorkflow: boolean }
  >(UPDATE_WORKFLOW_MUTATION, { id: cleanId, input }, opts.token, opts.tenantSlug);
}

export async function deleteWorkflow(
  id: string,
  opts: GqlOpts = {}
): Promise<void> {
  const cleanId = id?.trim();
  if (!cleanId) {
    throw new Error('Workflow ID is required');
  }
  await graphqlRequest<{ id: string }, { deleteWorkflow: boolean }>(
    DELETE_WORKFLOW_MUTATION,
    { id: cleanId },
    opts.token,
    opts.tenantSlug
  );
}

export async function activateWorkflow(
  id: string,
  opts: GqlOpts = {}
): Promise<void> {
  const cleanId = id?.trim();
  if (!cleanId) {
    throw new Error('Workflow ID is required');
  }
  await graphqlRequest<{ id: string }, { activateWorkflow: boolean }>(
    ACTIVATE_WORKFLOW_MUTATION,
    { id: cleanId },
    opts.token,
    opts.tenantSlug
  );
}

export async function pauseWorkflow(
  id: string,
  opts: GqlOpts = {}
): Promise<void> {
  const cleanId = id?.trim();
  if (!cleanId) {
    throw new Error('Workflow ID is required');
  }
  await graphqlRequest<{ id: string }, { pauseWorkflow: boolean }>(
    PAUSE_WORKFLOW_MUTATION,
    { id: cleanId },
    opts.token,
    opts.tenantSlug
  );
}

export async function triggerWorkflow(
  id: string,
  payload?: Record<string, unknown>,
  force?: boolean,
  opts: GqlOpts = {}
): Promise<string> {
  const cleanId = id?.trim();
  if (!cleanId) {
    throw new Error('Workflow ID is required');
  }
  const data = await graphqlRequest<
    { id: string; payload?: Record<string, unknown>; force?: boolean },
    { triggerWorkflow: string }
  >(
    TRIGGER_WORKFLOW_MUTATION,
    { id: cleanId, payload, force },
    opts.token,
    opts.tenantSlug
  );
  return data.triggerWorkflow;
}

export async function addWorkflowStep(
  workflowId: string,
  input: CreateStepInput,
  opts: GqlOpts = {}
): Promise<string> {
  const cleanId = workflowId?.trim();
  if (!cleanId) {
    throw new Error('Workflow ID is required');
  }
  const data = await graphqlRequest<
    { workflowId: string; input: CreateStepInput },
    { addWorkflowStep: string }
  >(ADD_STEP_MUTATION, { workflowId: cleanId, input }, opts.token, opts.tenantSlug);
  return data.addWorkflowStep;
}

export async function updateWorkflowStep(
  workflowId: string,
  stepId: string,
  input: UpdateStepInput,
  opts: GqlOpts = {}
): Promise<void> {
  const cleanWorkflowId = workflowId?.trim();
  const cleanStepId = stepId?.trim();
  if (!cleanWorkflowId || !cleanStepId) {
    throw new Error('Workflow ID and step ID are required');
  }
  await graphqlRequest<
    { workflowId: string; stepId: string; input: UpdateStepInput },
    { updateWorkflowStep: boolean }
  >(
    UPDATE_STEP_MUTATION,
    { workflowId: cleanWorkflowId, stepId: cleanStepId, input },
    opts.token,
    opts.tenantSlug
  );
}

export async function deleteWorkflowStep(
  workflowId: string,
  stepId: string,
  opts: GqlOpts = {}
): Promise<void> {
  const cleanWorkflowId = workflowId?.trim();
  const cleanStepId = stepId?.trim();
  if (!cleanWorkflowId || !cleanStepId) {
    throw new Error('Workflow ID and step ID are required');
  }
  await graphqlRequest<
    { workflowId: string; stepId: string },
    { deleteWorkflowStep: boolean }
  >(
    DELETE_STEP_MUTATION,
    { workflowId: cleanWorkflowId, stepId: cleanStepId },
    opts.token,
    opts.tenantSlug
  );
}

// ---------- Phase 4: Templates ----------

export const WORKFLOW_TEMPLATES_QUERY = `
query WorkflowTemplates {
  workflowTemplates {
    id
    name
    description
    category
  }
}`;

export const CREATE_FROM_TEMPLATE_MUTATION = `
mutation CreateWorkflowFromTemplate($templateId: String!, $name: String!) {
  createWorkflowFromTemplate(templateId: $templateId, name: $name)
}`;

export const GENERATE_WORKFLOW_MUTATION = `
mutation GenerateWorkflowFromDescription($description: String!) {
  generateWorkflowFromDescription(description: $description)
}`;

// ---------- Phase 4: Versions ----------

export const WORKFLOW_VERSIONS_QUERY = `
query WorkflowVersions($workflowId: UUID!) {
  workflowVersions(workflowId: $workflowId) {
    version
    createdAt
    createdBy
  }
}`;

export const RESTORE_VERSION_MUTATION = `
mutation RestoreWorkflowVersion($workflowId: UUID!, $version: Int!) {
  restoreWorkflowVersion(workflowId: $workflowId, version: $version)
}`;

// ---------- Phase 4 API functions ----------

export async function listWorkflowTemplates(
  opts: GqlOpts = {}
): Promise<WorkflowTemplate[]> {
  const data = await graphqlRequest<
    Record<string, never>,
    { workflowTemplates: WorkflowTemplate[] }
  >(WORKFLOW_TEMPLATES_QUERY, {}, opts.token, opts.tenantSlug);
  return data.workflowTemplates;
}

export async function createWorkflowFromTemplate(
  templateId: string,
  name: string,
  opts: GqlOpts = {}
): Promise<string> {
  const cleanTemplateId = templateId?.trim();
  const cleanName = name?.trim();
  if (!cleanTemplateId || !cleanName) {
    throw new Error('Template ID and workflow name are required');
  }
  const data = await graphqlRequest<
    { templateId: string; name: string },
    { createWorkflowFromTemplate: string }
  >(
    CREATE_FROM_TEMPLATE_MUTATION,
    { templateId: cleanTemplateId, name: cleanName },
    opts.token,
    opts.tenantSlug
  );
  return data.createWorkflowFromTemplate;
}

export async function generateWorkflowFromDescription(
  description: string,
  opts: GqlOpts = {}
): Promise<string> {
  const cleanDescription = description?.trim();
  if (!cleanDescription) {
    throw new Error('Description is required');
  }
  const data = await graphqlRequest<
    { description: string },
    { generateWorkflowFromDescription: string }
  >(
    GENERATE_WORKFLOW_MUTATION,
    { description: cleanDescription },
    opts.token,
    opts.tenantSlug
  );
  return data.generateWorkflowFromDescription;
}

export async function listWorkflowVersions(
  workflowId: string,
  opts: GqlOpts = {}
): Promise<WorkflowVersionSummary[]> {
  const cleanId = workflowId?.trim();
  if (!cleanId) {
    throw new Error('Workflow ID is required');
  }
  const data = await graphqlRequest<
    { workflowId: string },
    { workflowVersions: WorkflowVersionSummary[] }
  >(WORKFLOW_VERSIONS_QUERY, { workflowId: cleanId }, opts.token, opts.tenantSlug);
  return data.workflowVersions;
}

export async function restoreWorkflowVersion(
  workflowId: string,
  version: number,
  opts: GqlOpts = {}
): Promise<void> {
  const cleanId = workflowId?.trim();
  if (!cleanId || version <= 0) {
    throw new Error('Valid workflow ID and positive version number are required');
  }
  await graphqlRequest<
    { workflowId: string; version: number },
    { restoreWorkflowVersion: boolean }
  >(
    RESTORE_VERSION_MUTATION,
    { workflowId: cleanId, version },
    opts.token,
    opts.tenantSlug
  );
}
