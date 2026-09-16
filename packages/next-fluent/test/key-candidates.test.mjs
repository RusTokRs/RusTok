import test from 'node:test';
import assert from 'node:assert/strict';

import { buildKeyCandidates } from '../dist/utils.js';

test('dotted namespace candidates are unique and preserve lookup precedence', () => {
  assert.deepEqual(
    buildKeyCandidates('app.nav', 'dashboard'),
    ['app-nav-dashboard', 'app.nav.dashboard', 'dashboard']
  );
});

test('candidate generation never repeats equivalent aliases', () => {
  const cases = [
    ['app.nav', 'dashboard'],
    ['app.nav', 'account.email'],
    ['settings', 'account.email'],
    [undefined, 'plain.key'],
  ];

  for (const [namespace, key] of cases) {
    const candidates = buildKeyCandidates(namespace, key);
    assert.equal(
      new Set(candidates).size,
      candidates.length,
      `duplicate candidate for namespace=${namespace ?? '<none>'} key=${key}`
    );
  }
});

test('namespace whitespace is normalized once without changing aliases', () => {
  assert.deepEqual(
    buildKeyCandidates('  app.nav  ', 'dashboard'),
    ['app-nav-dashboard', 'app.nav.dashboard', 'dashboard']
  );
});
