import test from 'node:test';
import assert from 'node:assert/strict';
import {
  registerStorefrontModule,
  getModulesForSlot,
  normalizeStorefrontSlot,
} from './registry.ts';

test('normalizeStorefrontSlot normalizes snake_case to colon-separated', () => {
  assert.equal(normalizeStorefrontSlot('home_after_hero'), 'home:afterHero');
  assert.equal(normalizeStorefrontSlot('home_after_catalog'), 'home:afterCatalog');
  assert.equal(normalizeStorefrontSlot('home_before_footer'), 'home:beforeFooter');
  assert.equal(normalizeStorefrontSlot('header_navigation'), 'header:navigation');
  assert.equal(normalizeStorefrontSlot('header_actions'), 'header:actions');
  assert.equal(normalizeStorefrontSlot('footer_navigation'), 'footer:navigation');
  assert.equal(normalizeStorefrontSlot('checkout_payment_handoff'), 'checkout:paymentHandoff');
  assert.equal(normalizeStorefrontSlot('home:afterHero'), 'home:afterHero');
});

test('getModulesForSlot fails closed when enabledModules is undefined', () => {
  registerStorefrontModule({
    id: 'test-section-module',
    moduleSlug: 'test_module',
    slot: 'home:afterHero',
    order: 20,
    render: () => null,
  });

  registerStorefrontModule({
    id: 'test-section-core',
    slot: 'home:afterHero',
    order: 10,
    render: () => null,
  });

  // When enabledModules is undefined, only core modules without moduleSlug are returned
  const closed = getModulesForSlot('home:afterHero', undefined);
  assert.ok(closed.some((m) => m.id === 'test-section-core'));
  assert.ok(!closed.some((m) => m.id === 'test-section-module'));

  // When module is enabled, both are returned
  const enabled = getModulesForSlot('home:afterHero', ['test_module']);
  assert.ok(enabled.some((m) => m.id === 'test-section-core'));
  assert.ok(enabled.some((m) => m.id === 'test-section-module'));

  // Order is strictly ascending
  assert.equal(enabled[0].id, 'test-section-core');
  assert.equal(enabled[1].id, 'test-section-module');
});

test('getModulesForSlot matches normalized snake_case slot queries', () => {
  const fromSnake = getModulesForSlot('home_after_hero', ['test_module']);
  const fromColon = getModulesForSlot('home:afterHero', ['test_module']);
  assert.equal(fromSnake.length, fromColon.length);
});
