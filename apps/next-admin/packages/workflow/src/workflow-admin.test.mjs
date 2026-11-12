/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';

// Load api/workflows.ts by injecting mock graphqlRequest
const apiSource = readFileSync(
  new URL('./api/workflows.ts', import.meta.url),
  'utf8'
);
const cleanApiSource = stripTypeScriptTypes(
  apiSource
    .replace(
      "import { graphqlRequest, type GqlOpts } from '@/lib/graphql';",
      ''
    )
    .replace('export type { GqlOpts };', '')
    .replaceAll('export ', '')
);

const createApiModule = (mockGraphqlRequest) => {
  const exports = {};
  compileFunction(
    `${cleanApiSource}\nObject.assign(exports, {
      WORKFLOWS_QUERY,
      WORKFLOW_QUERY,
      WORKFLOW_EXECUTIONS_QUERY,
      CREATE_WORKFLOW_MUTATION,
      UPDATE_WORKFLOW_MUTATION,
      DELETE_WORKFLOW_MUTATION,
      ACTIVATE_WORKFLOW_MUTATION,
      PAUSE_WORKFLOW_MUTATION,
      TRIGGER_WORKFLOW_MUTATION,
      ADD_STEP_MUTATION,
      UPDATE_STEP_MUTATION,
      DELETE_STEP_MUTATION,
      WORKFLOW_TEMPLATES_QUERY,
      CREATE_FROM_TEMPLATE_MUTATION,
      GENERATE_WORKFLOW_MUTATION,
      WORKFLOW_VERSIONS_QUERY,
      RESTORE_VERSION_MUTATION,
      listWorkflows,
      getWorkflow,
      listWorkflowExecutions,
      createWorkflow,
      updateWorkflow,
      deleteWorkflow,
      activateWorkflow,
      pauseWorkflow,
      triggerWorkflow,
      addWorkflowStep,
      updateWorkflowStep,
      deleteWorkflowStep,
      listWorkflowTemplates,
      createWorkflowFromTemplate,
      generateWorkflowFromDescription,
      listWorkflowVersions,
      restoreWorkflowVersion
    });`,
    ['graphqlRequest', 'exports']
  )(mockGraphqlRequest, exports);
  return exports;
};

// Load nav.ts
const navSource = readFileSync(new URL('./nav.ts', import.meta.url), 'utf8');
const cleanNavSource = stripTypeScriptTypes(
  navSource
    .replace("import type { NavItem } from '@/types';", '')
    .replaceAll('export ', '')
);
const navModule = {};
compileFunction(
  `${cleanNavSource}\nnavModule.workflowNavItems = workflowNavItems;`,
  ['navModule']
)(navModule);
const { workflowNavItems } = navModule;

test('workflowNavItems specifies correct dashboard routes and keyboard shortcuts', () => {
  assert.equal(workflowNavItems.length, 1);
  const root = workflowNavItems[0];
  assert.equal(root.title, 'Workflows');
  assert.equal(root.group, 'modulePlugins');
  assert.equal(root.icon, 'workflow');
  assert.equal(root.items.length, 2);

  const [allWorkflows, newWorkflow] = root.items;
  assert.equal(allWorkflows.title, 'All Workflows');
  assert.equal(allWorkflows.url, '/dashboard/workflows');
  assert.deepEqual(allWorkflows.shortcut, ['w', 'l']);

  assert.equal(newWorkflow.title, 'New Workflow');
  assert.equal(newWorkflow.url, '/dashboard/workflows/new');
  assert.deepEqual(newWorkflow.shortcut, ['w', 'n']);
});

test('GraphQL operations expose required workflow queries and mutations', () => {
  const api = createApiModule(async () => ({}));
  assert.match(api.WORKFLOWS_QUERY, /query Workflows/);
  assert.match(api.WORKFLOW_QUERY, /query Workflow\(\$id: UUID!\)/);
  assert.match(
    api.WORKFLOW_EXECUTIONS_QUERY,
    /query WorkflowExecutions\(\$workflowId: UUID!\)/
  );
  assert.match(
    api.CREATE_WORKFLOW_MUTATION,
    /mutation CreateWorkflow\(\$input: GqlCreateWorkflowInput!\)/
  );
  assert.match(
    api.UPDATE_WORKFLOW_MUTATION,
    /mutation UpdateWorkflow\(\$id: UUID!, \$input: GqlUpdateWorkflowInput!\)/
  );
  assert.match(
    api.DELETE_WORKFLOW_MUTATION,
    /mutation DeleteWorkflow\(\$id: UUID!\)/
  );
  assert.match(
    api.ACTIVATE_WORKFLOW_MUTATION,
    /mutation ActivateWorkflow\(\$id: UUID!\)/
  );
  assert.match(
    api.PAUSE_WORKFLOW_MUTATION,
    /mutation PauseWorkflow\(\$id: UUID!\)/
  );
  assert.match(
    api.TRIGGER_WORKFLOW_MUTATION,
    /mutation TriggerWorkflow\(\$id: UUID!, \$payload: JSON, \$force: Boolean\)/
  );
  assert.match(
    api.ADD_STEP_MUTATION,
    /mutation AddWorkflowStep\(\$workflowId: UUID!, \$input: GqlCreateStepInput!\)/
  );
  assert.match(
    api.UPDATE_STEP_MUTATION,
    /mutation UpdateWorkflowStep\(\$workflowId: UUID!, \$stepId: UUID!, \$input: GqlUpdateStepInput!\)/
  );
  assert.match(
    api.DELETE_STEP_MUTATION,
    /mutation DeleteWorkflowStep\(\$workflowId: UUID!, \$stepId: UUID!\)/
  );
  assert.match(api.WORKFLOW_TEMPLATES_QUERY, /query WorkflowTemplates/);
  assert.match(
    api.CREATE_FROM_TEMPLATE_MUTATION,
    /mutation CreateWorkflowFromTemplate\(\$templateId: String!, \$name: String!\)/
  );
  assert.match(
    api.GENERATE_WORKFLOW_MUTATION,
    /mutation GenerateWorkflowFromDescription\(\$description: String!\)/
  );
  assert.match(
    api.WORKFLOW_VERSIONS_QUERY,
    /query WorkflowVersions\(\$workflowId: UUID!\)/
  );
  assert.match(
    api.RESTORE_VERSION_MUTATION,
    /mutation RestoreWorkflowVersion\(\$workflowId: UUID!, \$version: Int!\)/
  );
});

test('workflow lifecycle operations enforce non-empty identifier validation', async () => {
  const api = createApiModule(async () => ({
    createWorkflow: 'wf-1',
    updateWorkflow: true,
    deleteWorkflow: true,
    activateWorkflow: true,
    pauseWorkflow: true,
    triggerWorkflow: 'exec-1'
  }));

  // Create validation
  await assert.rejects(
    async () => api.createWorkflow({ name: '', triggerConfig: {} }),
    /Workflow name is required/
  );
  await assert.rejects(
    async () => api.createWorkflow({ name: '   ', triggerConfig: {} }),
    /Workflow name is required/
  );

  // Workflow ID validation
  await assert.rejects(
    async () => api.getWorkflow(''),
    /Workflow ID is required/
  );
  await assert.rejects(
    async () => api.updateWorkflow('', {}),
    /Workflow ID is required/
  );
  await assert.rejects(
    async () => api.deleteWorkflow(''),
    /Workflow ID is required/
  );
  await assert.rejects(
    async () => api.activateWorkflow(''),
    /Workflow ID is required/
  );
  await assert.rejects(
    async () => api.pauseWorkflow(''),
    /Workflow ID is required/
  );
  await assert.rejects(
    async () => api.triggerWorkflow(''),
    /Workflow ID is required/
  );
  await assert.rejects(
    async () => api.listWorkflowExecutions(''),
    /Workflow ID is required/
  );
});

test('step operations enforce non-empty workflowId and stepId validation', async () => {
  const api = createApiModule(async () => ({
    addWorkflowStep: 'step-1',
    updateWorkflowStep: true,
    deleteWorkflowStep: true
  }));

  await assert.rejects(
    async () =>
      api.addWorkflowStep('', {
        position: 1,
        stepType: 'ACTION',
        config: {},
        onError: 'STOP'
      }),
    /Workflow ID is required/
  );
  await assert.rejects(
    async () => api.updateWorkflowStep('', 'step-1', {}),
    /Workflow ID and step ID are required/
  );
  await assert.rejects(
    async () => api.updateWorkflowStep('wf-1', '', {}),
    /Workflow ID and step ID are required/
  );
  await assert.rejects(
    async () => api.deleteWorkflowStep('', 'step-1'),
    /Workflow ID and step ID are required/
  );
  await assert.rejects(
    async () => api.deleteWorkflowStep('wf-1', ''),
    /Workflow ID and step ID are required/
  );
});

test('templates and version rollbacks validate required arguments and positive version bounds', async () => {
  let capturedVars = null;
  const api = createApiModule(async (query, vars) => {
    capturedVars = vars;
    return {
      createWorkflowFromTemplate: 'wf-template-1',
      generateWorkflowFromDescription: 'wf-ai-1',
      restoreWorkflowVersion: true
    };
  });

  // Template instantiation
  await assert.rejects(
    async () => api.createWorkflowFromTemplate('', 'Name'),
    /Template ID and workflow name are required/
  );
  await assert.rejects(
    async () => api.createWorkflowFromTemplate('tmpl-1', ''),
    /Template ID and workflow name are required/
  );

  // Description generation
  await assert.rejects(
    async () => api.generateWorkflowFromDescription('   '),
    /Description is required/
  );

  // Version rollback
  await assert.rejects(
    async () => api.restoreWorkflowVersion('', 1),
    /Valid workflow ID and positive version number are required/
  );
  await assert.rejects(
    async () => api.restoreWorkflowVersion('wf-1', 0),
    /Valid workflow ID and positive version number are required/
  );
  await assert.rejects(
    async () => api.restoreWorkflowVersion('wf-1', -5),
    /Valid workflow ID and positive version number are required/
  );

  // Successful restore
  await api.restoreWorkflowVersion('wf-1', 3);
  assert.deepEqual(capturedVars, { workflowId: 'wf-1', version: 3 });
});
