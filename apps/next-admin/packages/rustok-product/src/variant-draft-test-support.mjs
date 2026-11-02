/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import vm from 'node:vm';
import { parsePriceAmount } from './money.ts';
import { chooseCombination, CombinationError } from './combinations.ts';

// Like the existing adapter tests, execute the actual owner-package core without
// evaluating the Next host/UI barrel. Only IO/framework imports are excluded.
const core = (file) =>
  stripTypeScriptTypes(readFileSync(new URL(file, import.meta.url), 'utf8'))
    .replace(/^import .* from .*;\s*$/gm, '')
    .replace(/^export /gm, '');
const policies = vm.runInNewContext(
  core('./schema-policy.ts') +
    '\n({ isAxisAttribute, isEligibleAxis, buildAxisPolicyPatch });',
  { Error }
);
export const axisPolicyCore = policies;
export const variantDraftCore = vm.runInNewContext(
  core('./variant-draft.ts') +
    '\n({ emptyInitialVariant, suggestedInitialAxes, validateAxisValues, normalizeInitialCatalog, initialVariantInput, setInitialAxis, setInitialAllowedValue, setInitialAssignment, appendInitialCombination, initialCombinationAxes });',
  {
    Error,
    Map,
    Set,
    parsePriceAmount,
    chooseCombination,
    CombinationError,
    isEligibleAxis: policies.isEligibleAxis
  }
);
