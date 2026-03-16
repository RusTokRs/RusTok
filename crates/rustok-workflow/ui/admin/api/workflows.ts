import { graphqlRequest } from '@/lib/graphql';

// ---------- GqlOpts ----------

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

// ---------- Types ----------

export type WorkflowStatus = 'DRAFT' | 'ACTIVE' | 'PAUSED' | 'ARCHIVED';
export type StepType = 'ACTION' | 'CONDITION' | 'DELAY' | 'ALLOY_SCRIPT' | 'EMIT_EVENT' | 'HTTP' | 'NOTIFY' | 'TRANSFORM';
export type OnError = 'STOP' | 'SKIP' | 'RETRY';
export type ExecutionStatus = 'RUNNING' | 'COMPLETED' | 'FAILED' | 'TIMED_OUT';
export type StepExecutionStatus = 'PENDING' | 'RUNNING' | 'COMPLETED' | 'FAILED' | 'SKIPPED';

export interface WorkflowSummary {
  id: string;
  tenantId: string;
  name: string;
  status: WorkflowStatus;
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
  createdBy: string | null;
  createdAt: string;
  updatedAt: string;
  failureCount: number;
  autoDisabledAt: string | null;
  steps: WorkflowStep[];
}

export interface StepExecution {
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
}

export interface UpdateWorkflowInput {
  name?: string;
  description?: string;
  status?: WorkflowStatus;
  triggerConfig?: Record<string, unknown>;
}

export interface CreateStepInput {
  position: number;
  stepType: StepType;
  config: Record<string, unknown>;
  onError: OnError;
  timeoutMs?: number;
}

export interface UpdateStepInput {
  position?: number;
  stepType?: StepType;
  config?: Record<string, unknown>;
  onError?: OnError;
  timeoutMs?: number;
}

// ---------- GraphQL operations (tenantId resolved server-side from headers) ----------

const WORKFLOWS_QUERY = `
query Workflows {
  workflows {
    id
    tenantId
    name
    status
    failureCount
    createdAt
    updatedAt
  }
}`;

const WORKFLOW_QUERY = `
query Workflow($id: UUID!) {
  workflow(id: $id) {
    id
    tenantId
    name
    description
    status
    triggerConfig
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

const WORKFLOW_EXECUTIONS_QUERY = `
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

const CREATE_WORKFLOW_MUTATION = `
mutation CreateWorkflow($input: GqlCreateWorkflowInput!) {
  createWorkflow(input: $input)
}`;

const UPDATE_WORKFLOW_MUTATION = `
mutation UpdateWorkflow($id: UUID!, $input: GqlUpdateWorkflowInput!) {
  updateWorkflow(id: $id, input: $input)
}`;

const DELETE_WORKFLOW_MUTATION = `
mutation DeleteWorkflow($id: UUID!) {
  deleteWorkflow(id: $id)
}`;

const ACTIVATE_WORKFLOW_MUTATION = `
mutation ActivateWorkflow($id: UUID!) {
  activateWorkflow(id: $id)
}`;

const PAUSE_WORKFLOW_MUTATION = `
mutation PauseWorkflow($id: UUID!) {
  pauseWorkflow(id: $id)
}`;

const TRIGGER_WORKFLOW_MUTATION = `
mutation TriggerWorkflow($id: UUID!, $payload: JSON, $force: Boolean) {
  triggerWorkflow(id: $id, payload: $payload, force: $force)
}`;

const ADD_STEP_MUTATION = `
mutation AddWorkflowStep($workflowId: UUID!, $input: GqlCreateStepInput!) {
  addWorkflowStep(workflowId: $workflowId, input: $input)
}`;

const UPDATE_STEP_MUTATION = `
mutation UpdateWorkflowStep($workflowId: UUID!, $stepId: UUID!, $input: GqlUpdateStepInput!) {
  updateWorkflowStep(workflowId: $workflowId, stepId: $stepId, input: $input)
}`;

const DELETE_STEP_MUTATION = `
mutation DeleteWorkflowStep($workflowId: UUID!, $stepId: UUID!) {
  deleteWorkflowStep(workflowId: $workflowId, stepId: $stepId)
}`;

// ---------- API functions ----------

export async function listWorkflows(opts: GqlOpts = {}): Promise<WorkflowSummary[]> {
  const data = await graphqlRequest<Record<string, never>, { workflows: WorkflowSummary[] }>(
    WORKFLOWS_QUERY, {}, opts.token, opts.tenantSlug
  );
  return data.workflows;
}

export async function getWorkflow(id: string, opts: GqlOpts = {}): Promise<WorkflowResponse | null> {
  const data = await graphqlRequest<{ id: string }, { workflow: WorkflowResponse | null }>(
    WORKFLOW_QUERY, { id }, opts.token, opts.tenantSlug
  );
  return data.workflow;
}

export async function listWorkflowExecutions(workflowId: string, opts: GqlOpts = {}): Promise<WorkflowExecution[]> {
  const data = await graphqlRequest<{ workflowId: string }, { workflowExecutions: WorkflowExecution[] }>(
    WORKFLOW_EXECUTIONS_QUERY, { workflowId }, opts.token, opts.tenantSlug
  );
  return data.workflowExecutions;
}

export async function createWorkflow(input: CreateWorkflowInput, opts: GqlOpts = {}): Promise<string> {
  const data = await graphqlRequest<{ input: CreateWorkflowInput }, { createWorkflow: string }>(
    CREATE_WORKFLOW_MUTATION, { input }, opts.token, opts.tenantSlug
  );
  return data.createWorkflow;
}

export async function updateWorkflow(id: string, input: UpdateWorkflowInput, opts: GqlOpts = {}): Promise<void> {
  await graphqlRequest<{ id: string; input: UpdateWorkflowInput }, { updateWorkflow: boolean }>(
    UPDATE_WORKFLOW_MUTATION, { id, input }, opts.token, opts.tenantSlug
  );
}

export async function deleteWorkflow(id: string, opts: GqlOpts = {}): Promise<void> {
  await graphqlRequest<{ id: string }, { deleteWorkflow: boolean }>(
    DELETE_WORKFLOW_MUTATION, { id }, opts.token, opts.tenantSlug
  );
}

export async function activateWorkflow(id: string, opts: GqlOpts = {}): Promise<void> {
  await graphqlRequest<{ id: string }, { activateWorkflow: boolean }>(
    ACTIVATE_WORKFLOW_MUTATION, { id }, opts.token, opts.tenantSlug
  );
}

export async function pauseWorkflow(id: string, opts: GqlOpts = {}): Promise<void> {
  await graphqlRequest<{ id: string }, { pauseWorkflow: boolean }>(
    PAUSE_WORKFLOW_MUTATION, { id }, opts.token, opts.tenantSlug
  );
}

export async function triggerWorkflow(
  id: string,
  payload?: Record<string, unknown>,
  force?: boolean,
  opts: GqlOpts = {}
): Promise<string> {
  const data = await graphqlRequest<
    { id: string; payload?: Record<string, unknown>; force?: boolean },
    { triggerWorkflow: string }
  >(TRIGGER_WORKFLOW_MUTATION, { id, payload, force }, opts.token, opts.tenantSlug);
  return data.triggerWorkflow;
}

export async function addWorkflowStep(workflowId: string, input: CreateStepInput, opts: GqlOpts = {}): Promise<string> {
  const data = await graphqlRequest<{ workflowId: string; input: CreateStepInput }, { addWorkflowStep: string }>(
    ADD_STEP_MUTATION, { workflowId, input }, opts.token, opts.tenantSlug
  );
  return data.addWorkflowStep;
}

export async function updateWorkflowStep(workflowId: string, stepId: string, input: UpdateStepInput, opts: GqlOpts = {}): Promise<void> {
  await graphqlRequest<{ workflowId: string; stepId: string; input: UpdateStepInput }, { updateWorkflowStep: boolean }>(
    UPDATE_STEP_MUTATION, { workflowId, stepId, input }, opts.token, opts.tenantSlug
  );
}

export async function deleteWorkflowStep(workflowId: string, stepId: string, opts: GqlOpts = {}): Promise<void> {
  await graphqlRequest<{ workflowId: string; stepId: string }, { deleteWorkflowStep: boolean }>(
    DELETE_STEP_MUTATION, { workflowId, stepId }, opts.token, opts.tenantSlug
  );
}
