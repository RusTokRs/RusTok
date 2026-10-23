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

import {
  CONNECTOR_MODES,
  isKnownMode,
  parseAddresses,
  formatSaveOutcome
} from './components/iggy-connector-model.ts';

// Load api/iggy-connector.ts with mock graphqlRequest
const apiSource = readFileSync(
  new URL('./api/iggy-connector.ts', import.meta.url),
  'utf8'
);
const cleanApiSource = stripTypeScriptTypes(
  apiSource
    .replace("import { graphqlRequest } from '@/lib/graphql';", '')
    .replaceAll('export ', '')
);
const createApiModule = (mockGraphqlRequest) => {
  const exports = {};
  compileFunction(
    `${cleanApiSource}\nObject.assign(exports, {
      CONFIGURATION_QUERY,
      UPDATE_MUTATION,
      getIggyConnectorConfiguration,
      saveIggyConnectorConfiguration
    });`,
    ['graphqlRequest', 'exports']
  )(mockGraphqlRequest, exports);
  return exports;
};

// Load nav.ts by stripping type import
const navSource = readFileSync(new URL('./nav.ts', import.meta.url), 'utf8');
const cleanNavSource = stripTypeScriptTypes(
  navSource
    .replace("import type { NavItem } from '@/types';", '')
    .replaceAll('export ', '')
);
const navModule = {};
compileFunction(
  `${cleanNavSource}\nnavModule.iggyConnectorNavItems = iggyConnectorNavItems;`,
  ['navModule']
)(navModule);
const { iggyConnectorNavItems } = navModule;

test('iggyConnectorNavItems specifies admin access and dashboard iggy-connector route', () => {
  assert.equal(iggyConnectorNavItems.length, 1);
  const root = iggyConnectorNavItems[0];
  assert.equal(root.title, 'Infrastructure');
  assert.deepEqual(root.access, { role: 'admin' });
  assert.equal(root.items.length, 1);
  assert.equal(root.items[0].title, 'Iggy Connector');
  assert.equal(root.items[0].url, '/dashboard/iggy-connector');
});

test('CONNECTOR_MODES contains bundled and external modes', () => {
  assert.equal(CONNECTOR_MODES.length, 2);
  assert.equal(CONNECTOR_MODES[0].mode, 'bundled');
  assert.equal(CONNECTOR_MODES[1].mode, 'external');
});

test('isKnownMode identifies valid modes', () => {
  assert.equal(isKnownMode('bundled'), true);
  assert.equal(isKnownMode('external'), true);
  assert.equal(isKnownMode('embedded'), false);
  assert.equal(isKnownMode(''), false);
});

test('parseAddresses handles empty strings, commas, newlines, and whitespace', () => {
  assert.deepEqual(parseAddresses(''), []);
  assert.deepEqual(parseAddresses('   \n  \t  \n '), []);
  assert.deepEqual(parseAddresses('127.0.0.1:8090'), ['127.0.0.1:8090']);
  assert.deepEqual(parseAddresses('127.0.0.1:8090, 192.168.1.1:8090'), [
    '127.0.0.1:8090',
    '192.168.1.1:8090'
  ]);
  assert.deepEqual(
    parseAddresses('127.0.0.1:8090, 10.0.0.1:8090\n\n192.168.1.5:8090,  '),
    ['127.0.0.1:8090', '10.0.0.1:8090', '192.168.1.5:8090']
  );
});

test('formatSaveOutcome formats user-facing message based on restartRequired', () => {
  assert.equal(
    formatSaveOutcome(true),
    'Saved. Restart the server to activate this connector mode.'
  );
  assert.equal(
    formatSaveOutcome(false),
    'Saved. This connector mode is already active.'
  );
});

test('getIggyConnectorConfiguration executes CONFIGURATION_QUERY with token and tenantSlug', async () => {
  let capturedQuery = null;
  let capturedToken = null;
  let capturedTenant = null;

  const mockGraphql = async (query, vars, token, tenantSlug) => {
    capturedQuery = query;
    capturedToken = token;
    capturedTenant = tenantSlug;
    return {
      iggyConnectorConfiguration: {
        activeMode: 'bundled',
        desiredMode: 'bundled',
        bundledAvailable: true,
        externalAddresses: ['127.0.0.1:8090'],
        externalUsername: 'iggy',
        passwordResolver: 'env',
        passwordKey: 'IGGY_PASSWORD',
        passwordConfigured: true,
        tlsEnabled: false,
        tlsDomain: null,
        configured: true,
        configurationError: null,
        restartRequired: false
      }
    };
  };

  const { getIggyConnectorConfiguration, CONFIGURATION_QUERY } =
    createApiModule(mockGraphql);
  const result = await getIggyConnectorConfiguration({
    token: 'jwt-123',
    tenantSlug: 'tenant-demo'
  });

  assert.equal(capturedQuery, CONFIGURATION_QUERY);
  assert.equal(capturedToken, 'jwt-123');
  assert.equal(capturedTenant, 'tenant-demo');
  assert.equal(result.activeMode, 'bundled');
  assert.equal(result.configured, true);
});

test('saveIggyConnectorConfiguration executes UPDATE_MUTATION with payload', async () => {
  let capturedMutation = null;
  let capturedVars = null;

  const mockGraphql = async (mutation, vars) => {
    capturedMutation = mutation;
    capturedVars = vars;
    return {
      updateIggyConnectorConfiguration: {
        desiredMode: 'external',
        configured: true,
        restartRequired: true
      }
    };
  };

  const { saveIggyConnectorConfiguration, UPDATE_MUTATION } =
    createApiModule(mockGraphql);
  const input = {
    mode: 'external',
    externalAddresses: ['192.168.1.10:8090'],
    externalUsername: 'admin',
    passwordResolver: 'env',
    passwordKey: 'IGGY_PASSWORD',
    tlsEnabled: true,
    tlsDomain: 'iggy.internal'
  };

  const outcome = await saveIggyConnectorConfiguration(input, {
    token: 'jwt-456'
  });

  assert.equal(capturedMutation, UPDATE_MUTATION);
  assert.deepEqual(capturedVars, { input });
  assert.equal(outcome.desiredMode, 'external');
  assert.equal(outcome.restartRequired, true);
});
