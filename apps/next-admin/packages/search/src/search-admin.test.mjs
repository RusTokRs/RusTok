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

// Load nav.ts
const navSource = readFileSync(new URL('./nav.ts', import.meta.url), 'utf8');
const cleanNavSource = stripTypeScriptTypes(
  navSource
    .replace("import type { NavItem } from '@/types';", '')
    .replaceAll('export ', '')
);
const navModule = {};
compileFunction(
  `${cleanNavSource}\nnavModule.searchNavItems = searchNavItems;`,
  ['navModule']
)(navModule);
const { searchNavItems } = navModule;

// Load pure exports from index.tsx by extracting the queries and helper functions
const indexSource = readFileSync(
  new URL('./index.tsx', import.meta.url),
  'utf8'
);

// Extract all exported queries/mutations and pure functions up to line 500
const pureSnippet = indexSource.slice(
  indexSource.indexOf('export const SEARCH_ADMIN_BOOTSTRAP_QUERY'),
  indexSource.indexOf('export function SearchAdminPage')
);

const cleanPureSnippet = stripTypeScriptTypes(
  pureSnippet.replaceAll('export ', '')
);

const searchExports = {};
compileFunction(
  `${cleanPureSnippet}\nObject.assign(searchExports, {
    SEARCH_ADMIN_BOOTSTRAP_QUERY,
    SEARCH_PREVIEW_QUERY,
    SEARCH_FILTER_PRESETS_QUERY,
    SEARCH_LAGGING_DOCUMENTS_QUERY,
    SEARCH_CONSISTENCY_ISSUES_QUERY,
    SEARCH_ANALYTICS_QUERY,
    SEARCH_DICTIONARY_SNAPSHOT_QUERY,
    TRIGGER_SEARCH_REBUILD_MUTATION,
    TRACK_SEARCH_CLICK_MUTATION,
    UPDATE_SEARCH_SETTINGS_MUTATION,
    UPSERT_SEARCH_SYNONYM_MUTATION,
    DELETE_SEARCH_SYNONYM_MUTATION,
    ADD_SEARCH_STOP_WORD_MUTATION,
    DELETE_SEARCH_STOP_WORD_MUTATION,
    UPSERT_SEARCH_PIN_RULE_MUTATION,
    DELETE_SEARCH_QUERY_RULE_MUTATION,
    parseCsv,
    optionalText,
    prettyJsonString,
    parseJsonValue,
    extractRankingProfileValue,
    extractSurfacePresetsJson,
    parseJsonArrayForEditor,
    mergeRelevanceEditorConfig,
    errorMessage
  });`,
  ['searchExports']
)(searchExports);

test('searchNavItems specifies correct dashboard route and plugin group', () => {
  assert.equal(searchNavItems.length, 1);
  const root = searchNavItems[0];
  assert.equal(root.title, 'Search');
  assert.equal(root.url, '/dashboard/search');
  assert.equal(root.group, 'modulePlugins');
  assert.equal(root.icon, 'search');
});

test('GraphQL queries and mutations expose required Search schema types and operations', () => {
  assert.match(
    searchExports.SEARCH_ADMIN_BOOTSTRAP_QUERY,
    /query SearchAdminBootstrap/
  );
  assert.match(
    searchExports.SEARCH_ADMIN_BOOTSTRAP_QUERY,
    /availableSearchEngines[\s\S]+searchDiagnostics/
  );
  assert.match(
    searchExports.SEARCH_PREVIEW_QUERY,
    /query SearchPreview\(\$input: SearchPreviewInput!\)/
  );
  assert.match(
    searchExports.SEARCH_FILTER_PRESETS_QUERY,
    /query SearchFilterPresets/
  );
  assert.match(
    searchExports.SEARCH_LAGGING_DOCUMENTS_QUERY,
    /query SearchLaggingDocuments/
  );
  assert.match(
    searchExports.SEARCH_CONSISTENCY_ISSUES_QUERY,
    /query SearchConsistencyIssues/
  );
  assert.match(searchExports.SEARCH_ANALYTICS_QUERY, /query SearchAnalytics/);
  assert.match(
    searchExports.SEARCH_DICTIONARY_SNAPSHOT_QUERY,
    /query SearchDictionarySnapshot/
  );
  assert.match(
    searchExports.TRIGGER_SEARCH_REBUILD_MUTATION,
    /mutation TriggerSearchRebuild/
  );
  assert.match(
    searchExports.UPDATE_SEARCH_SETTINGS_MUTATION,
    /mutation UpdateSearchSettings/
  );
  assert.match(
    searchExports.UPSERT_SEARCH_SYNONYM_MUTATION,
    /mutation UpsertSearchSynonym/
  );
  assert.match(
    searchExports.DELETE_SEARCH_SYNONYM_MUTATION,
    /mutation DeleteSearchSynonym/
  );
  assert.match(
    searchExports.ADD_SEARCH_STOP_WORD_MUTATION,
    /mutation AddSearchStopWord/
  );
  assert.match(
    searchExports.DELETE_SEARCH_STOP_WORD_MUTATION,
    /mutation DeleteSearchStopWord/
  );
  assert.match(
    searchExports.UPSERT_SEARCH_PIN_RULE_MUTATION,
    /mutation UpsertSearchPinRule/
  );
  assert.match(
    searchExports.DELETE_SEARCH_QUERY_RULE_MUTATION,
    /mutation DeleteSearchQueryRule/
  );
});

test('parseCsv handles empty values, spaces, and separators', () => {
  assert.deepEqual(searchExports.parseCsv(''), []);
  assert.deepEqual(searchExports.parseCsv('   '), []);
  assert.deepEqual(searchExports.parseCsv('apple, banana, cherry'), [
    'apple',
    'banana',
    'cherry'
  ]);
  assert.deepEqual(searchExports.parseCsv('apple,,  banana, ,'), [
    'apple',
    'banana'
  ]);
});

test('optionalText returns trimmed string or undefined for empty inputs', () => {
  assert.equal(searchExports.optionalText(''), undefined);
  assert.equal(searchExports.optionalText('   '), undefined);
  assert.equal(searchExports.optionalText('hello'), 'hello');
  assert.equal(searchExports.optionalText('  world  '), 'world');
});

test('prettyJsonString formats valid JSON and returns raw on invalid JSON', () => {
  assert.equal(searchExports.prettyJsonString('{"a":1}'), '{\n  "a": 1\n}');
  assert.equal(searchExports.prettyJsonString('not a json'), 'not a json');
});

test('parseJsonValue safely parses or returns null without throwing', () => {
  assert.deepEqual(searchExports.parseJsonValue('{"valid": true}'), {
    valid: true
  });
  assert.equal(searchExports.parseJsonValue('{invalid'), null);
});

test('extractRankingProfileValue resolves surface ranking with fallback', () => {
  const config = JSON.stringify({
    ranking_profiles: {
      search_preview: 'fast',
      admin_global_search: 'strict'
    }
  });

  assert.equal(
    searchExports.extractRankingProfileValue(config, 'search_preview'),
    'fast'
  );
  assert.equal(
    searchExports.extractRankingProfileValue(config, 'admin_global_search'),
    'strict'
  );
  assert.equal(
    searchExports.extractRankingProfileValue('{}', 'admin_global_search'),
    'exact'
  );
  assert.equal(
    searchExports.extractRankingProfileValue('{}', 'storefront_search'),
    'balanced'
  );
});

test('extractSurfacePresetsJson parses array presets or falls back to empty array', () => {
  const config = JSON.stringify({
    filter_presets: {
      storefront_search: [{ key: 'active', label: 'Active Only' }]
    }
  });

  assert.match(
    searchExports.extractSurfacePresetsJson(config, 'storefront_search'),
    /Active Only/
  );
  assert.equal(
    searchExports.extractSurfacePresetsJson('{}', 'storefront_search'),
    '[]'
  );
});

test('parseJsonArrayForEditor validates that input is a valid JSON array', () => {
  assert.deepEqual(
    searchExports.parseJsonArrayForEditor('Presets', '[1, 2, 3]'),
    [1, 2, 3]
  );
  assert.throws(
    () => searchExports.parseJsonArrayForEditor('Presets', '{"not": "array"}'),
    /Presets must be a JSON array\./
  );
  assert.throws(
    () => searchExports.parseJsonArrayForEditor('Presets', '{invalid json'),
    /Presets must be valid JSON/
  );
});

test('mergeRelevanceEditorConfig constructs and validates complete settings JSON', () => {
  const merged = searchExports.mergeRelevanceEditorConfig({
    configText: '{}',
    rankingDefault: 'balanced',
    rankingPreview: 'fast',
    rankingStorefront: 'relevance',
    rankingAdminGlobal: 'exact',
    previewPresets: '[]',
    storefrontPresets: '[{"key":"in_stock","label":"In Stock"}]'
  });

  const parsed = JSON.parse(merged);
  assert.equal(parsed.ranking_profiles.default, 'balanced');
  assert.equal(parsed.ranking_profiles.search_preview, 'fast');
  assert.equal(parsed.ranking_profiles.storefront_search, 'relevance');
  assert.equal(parsed.ranking_profiles.admin_global_search, 'exact');
  assert.equal(parsed.filter_presets.storefront_search[0].key, 'in_stock');

  // Throws if root is not an object
  assert.throws(
    () =>
      searchExports.mergeRelevanceEditorConfig({
        configText: '[1, 2, 3]',
        rankingDefault: 'd',
        rankingPreview: 'p',
        rankingStorefront: 's',
        rankingAdminGlobal: 'a',
        previewPresets: '[]',
        storefrontPresets: '[]'
      }),
    /Settings config root must be a JSON object\./
  );
});
