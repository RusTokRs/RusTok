/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import vm from 'node:vm';

const source = readFileSync(
  new URL('../../../src/shared/api/graphql.ts', import.meta.url),
  'utf8'
);
const executable = source
  .replace(/^import[\s\S]*?;\s*/gm, '')
  .replace(/\bexport /g, '');
function transport(result, reject = false) {
  const calls = [];
  class ApolloClient {
    mutate(options) {
      calls.push(options);
      return reject ? Promise.reject(result) : Promise.resolve(result);
    }
    query(options) {
      calls.push(options);
      return reject ? Promise.reject(result) : Promise.resolve(result);
    }
  }
  const loaded = vm.runInNewContext(
    stripTypeScriptTypes(executable) + '\n({graphqlRequest, GraphqlError});',
    {
      ApolloClient,
      InMemoryCache: class {},
      HttpLink: class {},
      gql: (query) => query,
      CombinedGraphQLErrors: { is: (error) => Array.isArray(error?.errors) },
      process: { env: {} },
      fetch: () => assert.fail('unit executor must intercept IO'),
      Buffer,
      Error
    }
  );
  return { ...loaded, calls };
}

test('Apollo 4 combined errors preserve owner code and boolean retryability', async () => {
  const executor = transport({
    error: {
      message: 'Combined error',
      errors: [
        {
          message: 'Owner refusal',
          extensions: {
            code: 'PRODUCT_VALIDATION',
            retryable: false,
            outcome: 'rejected'
          }
        }
      ]
    }
  });
  await assert.rejects(
    executor.graphqlRequest('mutation Test { test }'),
    (error) => {
      assert.ok(error instanceof executor.GraphqlError);
      assert.equal(error.code, 'PRODUCT_VALIDATION');
      assert.equal(error.retryable, false);
      assert.equal(error.outcome, 'rejected');
      return true;
    }
  );
  assert.equal(executor.calls[0].errorPolicy, 'all');
});

test('partial GraphQL data with owner errors must not become successful acknowledgement', async () => {
  const executor = transport({
    data: { id: 'product' },
    error: {
      errors: [
        {
          message: 'Retry later',
          extensions: {
            code: 'PRODUCT_TEMPORARILY_UNAVAILABLE',
            retryable: true
          }
        }
      ]
    }
  });
  await assert.rejects(
    executor.graphqlRequest('mutation Test { test }'),
    (error) => error.retryable === true
  );
});

test('network failure carries no invented definitive refusal', async () => {
  const executor = transport(new Error('network failed'), true);
  await assert.rejects(
    executor.graphqlRequest('mutation Test { test }'),
    (error) => error.retryable === undefined
  );
});

test('authentication refusal does not manufacture owner retryability', async () => {
  const executor = transport({
    error: {
      errors: [
        {
          message: 'unauthorized',
          extensions: { code: 'UNAUTHORIZED', retryable: false }
        }
      ]
    }
  });
  await assert.rejects(
    executor.graphqlRequest('mutation Test { test }'),
    (error) => error.code === 'UNAUTHORIZED' && error.retryable === undefined
  );
});

test('missing data without owner metadata is an unknown outcome', async () => {
  const executor = transport({ data: null });
  await assert.rejects(
    executor.graphqlRequest('mutation Test { test }'),
    (error) => error.retryable === undefined
  );
});
