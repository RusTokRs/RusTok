import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import {
  FbaRegistryValidationError,
  FBA_PROVIDER_REGISTRY_SCHEMA_PATH,
  validateProviderRegistryBaseline,
} from './fba-registry-validation.mjs';

const registryPath = 'crates/modules/rustok-rbac/contracts/rbac-fba-registry.json';
const registry = () => JSON.parse(readFileSync(registryPath, 'utf8'));
const expected = {
  expectedModule: 'rbac',
  expectedContractVersion: 'rbac.permission_decision.v1',
  allowedStatuses: ['in_progress', 'boundary_ready'],
};

test('accepts the RBAC provider registry against the shared v1 schema', () => {
  assert.equal(
    FBA_PROVIDER_REGISTRY_SCHEMA_PATH,
    'crates/libs/rustok-fba/contracts/provider-registry-v1.schema.json',
  );
  assert.doesNotThrow(() => validateProviderRegistryBaseline({ registry: registry(), ...expected }));
});

test('rejects a provider registry without canonical port context metadata', () => {
  const invalid = registry();
  delete invalid.ports[0].context;

  assert.throws(
    () => validateProviderRegistryBaseline({ registry: invalid, ...expected }),
    (error) =>
      error instanceof FbaRegistryValidationError
      && error.message.includes('$.ports[0]: missing required property context'),
  );
});

test('rejects duplicate provider operations', () => {
  const invalid = registry();
  invalid.ports[0].operations.push('check_permissions');

  assert.throws(
    () => validateProviderRegistryBaseline({ registry: invalid, ...expected }),
    (error) => error instanceof FbaRegistryValidationError && error.message.includes('$.ports[0].operations: values must be unique'),
  );
});

test('rejects contract-test operations that are absent from every provider port', () => {
  const invalid = registry();
  invalid.contract_tests.cases[0].operation = 'resolve_everything';

  assert.throws(
    () => validateProviderRegistryBaseline({ registry: invalid, ...expected }),
    (error) => error instanceof FbaRegistryValidationError && error.message.includes('rbac.resolve_everything is not declared by a provider port'),
  );
});
