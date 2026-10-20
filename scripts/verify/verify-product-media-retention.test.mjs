import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { files, verify } from './verify-product-media-retention.mjs';
const sources = Object.fromEntries(
  Object.entries(files).map(([key, file]) => [key, readFileSync(file, 'utf8')])
);
test('current durable Media implementation satisfies architectural guards', () =>
  assert.deepEqual(verify(sources), []));
for (const [name, file, needle, pattern] of [
  ['non-atomic worker startup', 'dispatcher', 'shared_insert_if_absent(ProductMediaReleaseWorkerReservation)', /atomically reserve/],
  ['split delivery authority', 'runtime', '.process_media_releases(batch_limit)', /command owner catalog/],
  ['embedded Media-only worker', 'services', '#[cfg(feature = "mod-product")]\npub mod product_media_release_worker;', /must not require the embedded Media/],
  ['maintenance permission classification', 'reference', 'PortError::forbidden(', /permission denial must stay Forbidden/],
  ['maintenance HTTP denial classification', 'operator', 'PortErrorKind::Forbidden => HttpError::forbidden(', /preserve owner Forbidden/],
  ['hold audit roundtrips', 'reference', 'local_images.get(&hold.image_id)', /in a batch/],

  ['stale worker retry overwrite', 'reference', 'Attempts.eq(job.attempts)', /compare retry generation/],
  ['missing native owner integration', 'integration', 'native_owner_fences_gc_and_refuses_historical_hold_acknowledgements', /native Media owner integration/],
  ['missing loopback owner integration', 'integration', 'loopback_owner_fences_gc_and_refuses_historical_hold_acknowledgements', /real loopback Media owner integration/],
  [
    'unstable image ID',
    'commands',
    'current_product_operation_id().unwrap_or_else(generate_id)',
    /identity must be stable/
  ],
  [
    'missing rollout barrier',
    'commands',
    'has_unverified_media_images',
    /legacy backfill must gate/
  ],
  [
    'missing image retain',
    'commands',
    'self.retain_product_image',
    /retain must precede/
  ],
  [
    'image deletion drops release work',
    'commands',
    'self.enqueue_product_media_release',
    /durable release intent/
  ],
  [
    'pre-replay decorator',
    'runtime',
    'ProductMediaReferencePolicy::Unavailable',
    /missing capability/
  ],
  [
    'unbounded media calls',
    'reference',
    'tokio::time::timeout(',
    /enforced deadlines/
  ],
  [
    'queue starvation',
    'reference',
    'Utc::now() + ChronoDuration::seconds(delay)',
    /back off/
  ],
  ['unsafe release', 'reference', 'live.is_none()', /reject live relations/],
  [
    'stale release receipt',
    'reference',
    'port.lookup(context, vec![job.image_id])',
    /current owner absence/
  ],
  [
    'stale retain receipt',
    'host',
    'holds.len() != 1 || holds[0] != retained',
    /historical retain/
  ],
  [
    'mixed authorities',
    'host',
    'shared_get::<MediaAssetPorts>()',
    /atomic Media owner pair/
  ],
  [
    'owner table coupling',
    'reference',
    'pub trait ProductMediaReferencePort',
    /foreign persistence/
  ],
  [
    'unregistered migration',
    'migrations',
    'Box::new(m20261018_000037_create_product_media_releases::Migration)',
    /migration must be registered/
  ],
  [
    'missing background delivery',
    'worker',
    'runtime.process_media_releases(100)',
    /actually drain/
  ],
  [
    'premature readiness seal',
    'reference',
    'if !report.legacy_images_remaining',
    /only seal after full legacy/
  ],
  [
    'missing steady-state readiness',
    'reference',
    'product_media_retention_readiness::Entity::find_by_id(tenant_id)',
    /indexed readiness/
  ],
  ['unguarded operator', 'operator', 'PRODUCTS_MANAGE', /enforce RBAC/],
  [
    'unreachable backfill',
    'routes',
    'products/media-references/backfill',
    /backfill must be reachable/
  ]
]) {
  test(`guard rejects ${name}`, () => {
    assert.ok(
      sources[file].includes(needle),
      `mutation target missing: ${needle}`
    );
    let changed = sources[file].replaceAll(
      needle,
      name === 'owner table coupling' ? 'rustok_media::MediaService' : 'REMOVED'
    );
    const errors = verify({ ...sources, [file]: changed });
    assert.match(errors.join('\n'), pattern);
  });
}
