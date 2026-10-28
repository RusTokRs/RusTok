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
  isRestartRequired,
  evaluateProfileSelection,
  formatSaveOutcomeKey
} from './components/events-model.ts';

// Load api/events.ts by injecting mock graphqlRequest
const apiSource = readFileSync(
  new URL('./api/events.ts', import.meta.url),
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
      EVENTS_STATUS_QUERY,
      EVENT_DELIVERY_CONFIGURATION_QUERY,
      UPDATE_EVENT_DELIVERY_CONFIGURATION_MUTATION,
      getEventsStatus,
      getEventDeliveryConfiguration,
      saveEventDeliveryProfile
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
  `${cleanNavSource}\nnavModule.eventsNavItems = eventsNavItems;`,
  ['navModule']
)(navModule);
const { eventsNavItems } = navModule;

test('eventsNavItems specifies admin access and correct dashboard route', () => {
  assert.equal(eventsNavItems.length, 1);
  const root = eventsNavItems[0];
  assert.equal(root.title, 'Infrastructure');
  assert.deepEqual(root.access, { role: 'admin' });
  assert.equal(root.items.length, 1);
  assert.equal(root.items[0].title, 'Events & Outbox');
  assert.equal(root.items[0].url, '/dashboard/events');
});

test('isRestartRequired correctly evaluates profile drift and server flag', () => {
  // Matching active profile and no pending restart -> false
  assert.equal(isRestartRequired('outbox', 'outbox', false), false);

  // Profile differs from active -> true
  assert.equal(isRestartRequired('outbox_iggy', 'outbox', false), true);
  assert.equal(isRestartRequired('outbox', 'outbox_iggy', false), true);

  // Server already requires restart -> always true
  assert.equal(isRestartRequired('outbox', 'outbox', true), true);
  assert.equal(isRestartRequired('outbox_iggy', 'outbox', true), true);
});

test('evaluateProfileSelection enforces Iggy connector prerequisite', () => {
  // outbox_iggy when Iggy connector is not configured -> must prompt configuration
  const unconfiguredIggy = evaluateProfileSelection('outbox_iggy', {
    activeProfile: 'outbox',
    iggyConfigured: false,
    serverRestartRequired: false
  });
  assert.equal(unconfiguredIggy.canApply, false);
  assert.equal(unconfiguredIggy.showIggyConfiguration, true);

  // outbox_iggy when Iggy connector is configured -> succeeds with restart required
  const configuredIggy = evaluateProfileSelection('outbox_iggy', {
    activeProfile: 'outbox',
    iggyConfigured: true,
    serverRestartRequired: false
  });
  assert.equal(configuredIggy.canApply, true);
  assert.equal(configuredIggy.showIggyConfiguration, false);
  assert.equal(configuredIggy.restartRequired, true);

  // standard outbox -> succeeds regardless of Iggy state
  const outboxSelection = evaluateProfileSelection('outbox', {
    activeProfile: 'outbox',
    iggyConfigured: false,
    serverRestartRequired: false
  });
  assert.equal(outboxSelection.canApply, true);
  assert.equal(outboxSelection.showIggyConfiguration, false);
  assert.equal(outboxSelection.restartRequired, false);
});

test('formatSaveOutcomeKey distinguishes restart required from no restart', () => {
  assert.equal(formatSaveOutcomeKey(true), 'saved');
  assert.equal(formatSaveOutcomeKey(false), 'savedWithoutRestart');
});

test('GraphQL queries expose the required schema fields', () => {
  const api = createApiModule(async () => ({}));

  // Status query fields
  assert.match(api.EVENTS_STATUS_QUERY, /eventsStatus/);
  assert.match(api.EVENTS_STATUS_QUERY, /configuredProfile/);
  assert.match(api.EVENTS_STATUS_QUERY, /iggyMode/);
  assert.match(api.EVENTS_STATUS_QUERY, /relayIntervalMs/);
  assert.match(api.EVENTS_STATUS_QUERY, /dlqEnabled/);
  assert.match(api.EVENTS_STATUS_QUERY, /maxAttempts/);
  assert.match(api.EVENTS_STATUS_QUERY, /pendingEvents/);
  assert.match(api.EVENTS_STATUS_QUERY, /dlqEvents/);
  assert.match(api.EVENTS_STATUS_QUERY, /availableTransports/);

  // Configuration query fields
  assert.match(
    api.EVENT_DELIVERY_CONFIGURATION_QUERY,
    /eventDeliveryConfiguration/
  );
  assert.match(api.EVENT_DELIVERY_CONFIGURATION_QUERY, /activeProfile/);
  assert.match(api.EVENT_DELIVERY_CONFIGURATION_QUERY, /desiredProfile/);
  assert.match(api.EVENT_DELIVERY_CONFIGURATION_QUERY, /iggyMode/);
  assert.match(api.EVENT_DELIVERY_CONFIGURATION_QUERY, /iggyConfigured/);
  assert.match(api.EVENT_DELIVERY_CONFIGURATION_QUERY, /restartRequired/);

  // Mutation fields
  assert.match(
    api.UPDATE_EVENT_DELIVERY_CONFIGURATION_MUTATION,
    /updateEventDeliveryConfiguration/
  );
  assert.match(
    api.UPDATE_EVENT_DELIVERY_CONFIGURATION_MUTATION,
    /desiredProfile/
  );
  assert.match(
    api.UPDATE_EVENT_DELIVERY_CONFIGURATION_MUTATION,
    /restartRequired/
  );
});

test('saveEventDeliveryProfile sends profile input and returns clean update outcome', async () => {
  let calledQuery = null;
  let calledVars = null;
  let calledToken = null;
  let calledTenant = null;

  const mockGraphqlRequest = async (query, vars, token, tenant) => {
    calledQuery = query;
    calledVars = vars;
    calledToken = token;
    calledTenant = tenant;
    return {
      updateEventDeliveryConfiguration: {
        desiredProfile: 'outbox_iggy',
        restartRequired: true
      }
    };
  };

  const api = createApiModule(mockGraphqlRequest);
  const result = await api.saveEventDeliveryProfile('outbox_iggy', {
    token: 'test-token',
    tenantSlug: 'test-tenant'
  });

  assert.equal(result.desiredProfile, 'outbox_iggy');
  assert.equal(result.restartRequired, true);
  assert.match(calledQuery, /updateEventDeliveryConfiguration/);
  assert.deepEqual(calledVars, { input: { profile: 'outbox_iggy' } });
  assert.equal(calledToken, 'test-token');
  assert.equal(calledTenant, 'test-tenant');
});

test('getEventsStatus propagates token and tenantSlug to gateway', async () => {
  const mockStatus = {
    configuredProfile: 'outbox',
    iggyMode: 'none',
    relayIntervalMs: 5000,
    dlqEnabled: true,
    maxAttempts: 5,
    pendingEvents: 12,
    dlqEvents: 0,
    availableTransports: ['outbox']
  };

  const api = createApiModule(async (query, vars, token, tenant) => {
    assert.match(query, /eventsStatus/);
    assert.equal(token, 'auth-token');
    assert.equal(tenant, 'my-tenant');
    return { eventsStatus: mockStatus };
  });

  const res = await api.getEventsStatus({
    token: 'auth-token',
    tenantSlug: 'my-tenant'
  });
  assert.deepEqual(res, mockStatus);
});
