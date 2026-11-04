import test from 'node:test';
import assert from 'node:assert/strict';
import {
  registerAdminComponent,
  getAdminComponentsForSlot,
  normalizeAdminSlot,
} from './registry.ts';

test('normalizeAdminSlot normalizes snake_case to colon-separated', () => {
  assert.equal(normalizeAdminSlot('dashboard_metric'), 'dashboard:metric');
  assert.equal(normalizeAdminSlot('dashboard_section'), 'dashboard:section');
  assert.equal(normalizeAdminSlot('header_actions'), 'header:actions');
  assert.equal(normalizeAdminSlot('sidebar_footer'), 'sidebar:footer');
  assert.equal(normalizeAdminSlot('dashboard:metric'), 'dashboard:metric');
});

test('getAdminComponentsForSlot fails closed when enabledModules is undefined', () => {
  registerAdminComponent({
    id: 'test-metric-module',
    moduleSlug: 'test_module',
    slot: 'dashboard:metric',
    order: 10,
    render: () => null,
  });

  registerAdminComponent({
    id: 'test-metric-core',
    slot: 'dashboard:metric',
    order: 5,
    render: () => null,
  });

  // When enabledModules is undefined, only core components without moduleSlug are returned
  const closed = getAdminComponentsForSlot('dashboard:metric', undefined);
  assert.ok(closed.some((c) => c.id === 'test-metric-core'));
  assert.ok(!closed.some((c) => c.id === 'test-metric-module'));

  // When module is enabled, both are returned
  const enabled = getAdminComponentsForSlot('dashboard:metric', ['test_module']);
  assert.ok(enabled.some((c) => c.id === 'test-metric-core'));
  assert.ok(enabled.some((c) => c.id === 'test-metric-module'));

  // Ordering is preserved
  assert.equal(enabled[0].id, 'test-metric-core');
  assert.equal(enabled[1].id, 'test-metric-module');
});

test('getAdminComponentsForSlot matches normalized snake_case slot queries', () => {
  const fromSnake = getAdminComponentsForSlot('dashboard_metric', ['test_module']);
  const fromColon = getAdminComponentsForSlot('dashboard:metric', ['test_module']);
  assert.equal(fromSnake.length, fromColon.length);
});
