#!/usr/bin/env node
// Architectural regression checks, NOT runtime/transaction proof.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
export const files = {
  integration: 'apps/server/src/services/product_media_reference_integration_tests.rs',
  services: 'apps/server/src/services/mod.rs',
  consumer:
    'crates/modules/rustok-product/contracts/product-media-consumer-registry.json',
  manifest: 'crates/modules/rustok-product/rustok-module.toml',
  reference: 'crates/modules/rustok-product/src/media_reference.rs',
  commands: 'crates/modules/rustok-product/src/services/catalog/commands.rs',
  admission: 'crates/modules/rustok-product/src/catalog_command_port.rs',
  runtime: 'crates/modules/rustok-product/src/runtime.rs',
  catalog: 'crates/modules/rustok-product/src/services/catalog.rs',
  host: 'apps/server/src/services/product_media_reference.rs',
  worker: 'apps/server/src/services/product_media_release_worker.rs',
  dispatcher: 'apps/server/src/services/module_event_dispatcher.rs',
  migration:
    'crates/modules/rustok-product/src/migrations/m20261018_000037_create_product_media_releases.rs',
  migrations: 'crates/modules/rustok-product/src/migrations/mod.rs',
  media: 'crates/modules/rustok-media/src/ports.rs',
  operator:
    'crates/modules/rustok-commerce/src/controllers/admin/product_media.rs',
  routes: 'crates/modules/rustok-commerce/src/controllers/admin/mod.rs',
  context: 'crates/modules/rustok-commerce/src/controllers/products.rs',
  openapi: 'crates/modules/rustok-commerce/src/openapi.rs'
};
export function verify(s) {
  const errors = [];
  const has = (key, expression, reason) => {
    if (!expression.test(s[key] ?? '')) errors.push(reason);
  };
  const absent = (key, expression, reason) => {
    if (expression.test(s[key] ?? '')) errors.push(reason);
  };
  const method = (text, name, next) =>
    text.slice(
      text.indexOf(`pub async fn ${name}(`),
      next ? text.indexOf(`pub async fn ${next}(`) : undefined
    );
  const ordered = (text, a, b, reason) => {
    if (
      text.indexOf(a) < 0 ||
      text.indexOf(b) < 0 ||
      text.indexOf(a) >= text.indexOf(b)
    )
      errors.push(reason);
  };
  const add = method(s.commands, 'add_product_image', 'update_product_image');
  ordered(
    add,
    'current_product_operation_id().unwrap_or_else(generate_id)',
    'self.retain_product_image',
    'image identity must be stable before retain'
  );
  ordered(
    add,
    'has_unverified_media_images',
    'self.retain_product_image',
    'legacy backfill must gate new image admission'
  );
  ordered(
    add,
    'self.retain_product_image',
    'ProductWriteTransaction::begin',
    'retain must precede the Product transaction'
  );
  ordered(
    add,
    'ProductWriteTransaction::begin',
    'find_product_for_update_in_tx',
    'parent must be rechecked under the transaction lock'
  );
  ordered(
    add,
    'mark_image_media_provisioned',
    'txn.commit()',
    'coverage marker must commit with the image'
  );
  for (const [name, next] of [
    ['delete_product', 'create_variant'],
    ['delete_product_image', 'reorder_product_images']
  ]) {
    const deletion = method(s.commands, name, next);
    ordered(
      deletion,
      'enqueue_product_media_release',
      'product_image_translation::Entity::delete_many',
      `${name}: durable release intent must precede local removal`
    );
    ordered(
      deletion,
      'enqueue_product_media_release',
      'txn.commit()',
      `${name}: queue and removal must share the transaction`
    );
  }
  absent(
    'commands',
    /\.release_asset_reference\(|\.release\(/,
    'image commands must never perform precommit/compensating release'
  );
  has(
    'admission',
    /with_media_context\(\s*context\.clone\(\),\s*with_product_operation_receipt\(lease, future\)/s,
    'Media context must surround admitted execution, not replay'
  );
  has(
    'catalog',
    /bound_media_reference_port\(port\)/,
    'direct internal callers also need bounded Media IO'
  );
  absent(
    'reference',
    /rustok_media|DELETE FROM media_|MediaService::/,
    'Product must not depend on foreign persistence or owner services'
  );
  has(
    'reference',
    /tokio::time::timeout\(/,
    'all Media operations require enforced deadlines'
  );
  has(
    'reference',
    /min\(DEADLINE\.as_millis\(\) as u64\)/,
    'caller deadlines must be capped'
  );
  has(
    'reference',
    /product_media_release::ActiveModel[\s\S]*?\.insert\(txn\)/,
    'release work must be inserted through the caller transaction'
  );
  has(
    'reference',
    /NextAttemptAt\.lte\(Utc::now\(\)\)/,
    'worker must select only due work'
  );
  has('reference', /\.limit\(batch_limit\)/, 'worker batches must be bounded');
  has(
    'reference',
    /Utc::now\(\) \+ ChronoDuration::seconds\(delay\)/,
    'failed release must back off without losing durable work'
  );
  has(
    'reference',
    /live\.is_none\(\).*?Some\(job\.tenant_id\)/,
    'worker must reject live relations and mismatched tenant context'
  );
  has(
    'reference',
    /port\.lookup\(context, vec!\[job.image_id\]\)/,
    'release acknowledgement requires current owner absence proof'
  );
  has(
    'reference',
    /legacy_images_remaining/,
    'backfill must expose unresolved legacy coverage'
  );
  has(
    'reference',
    /PRODUCTS_MANAGE/,
    'backfill needs owner-side operator permission'
  );
  const audit = s.reference.slice(
    s.reference.indexOf('pub async fn audit_media_references'),
    s.reference.indexOf('pub async fn backfill_media_references')
  );
  if (/\.release\(|\.retain\(|\.insert\(|\.delete\(/.test(audit))
    errors.push('audit must be diagnostic only');
  has('reference', /missing_media_hold/, 'audit must report missing holds');
  has(
    'reference',
    /media_reference_mismatch/,
    'audit must report identity mismatches'
  );
  has(
    'reference',
    /orphan_media_hold/,
    'audit must report conservative orphans'
  );
  absent(
    'runtime',
    /ProductMediaValidatedCommandPort|OpaqueReferences/,
    'legacy pre-replay/opaque path must be removed'
  );
  has(
    'runtime',
    /self.command_port = Arc::new\(catalog.clone\(\)\)/,
    'retention must be installed inside the embedded owner'
  );
  has(
    'runtime',
    /ProductMediaReferencePolicy::Unavailable/,
    'missing capability must fail closed'
  );
  has(
    'host',
    /runtime.profile\(\) == ProductCatalogCommandProfile::External/,
    'external owner replay must not be locally decorated'
  );
  has(
    'host',
    /shared_get::<MediaAssetPorts>\(\)/,
    'host must resolve an atomic Media owner pair'
  );
  has(
    'host',
    /\.retain_asset_reference\(/,
    'host must retain through the public Media write port'
  );
  has(
    'host',
    /holds.len\(\) != 1 \|\| holds\[0\] != retained/,
    'historical retain acknowledgement must be confirmed against current owner state'
  );
  has('host', /Sha256::digest/, 'child keys must be bounded and deterministic');
  has(
    'media',
    /new<T: MediaAssetReadPort \+ MediaAssetWritePort \+ 'static>/,
    'paired ports must originate from a single owner instance'
  );
  has(
    'migration',
    /product_media_releases/,
    'durable queue migration is mandatory'
  );
  has(
    'migration',
    /product_image_media_coverage/,
    'existing images need explicit provisioning coverage'
  );
  has(
    'migration',
    /ForeignKeyAction::Cascade/,
    'coverage markers must follow Product relation removal'
  );
  has(
    'migrations',
    /Box::new\(m20261018_000037_create_product_media_releases::Migration\)/,
    'migration must be registered'
  );
  has(
    'dispatcher',
    /runs_background_workers\(\)/,
    'worker must respect the process role'
  );
  has(
    'dispatcher',
    /shared_insert_if_absent\(ProductMediaReleaseWorkerReservation\)/,
    'worker must atomically reserve a singleton per process'
  );
  has(
    'dispatcher',
    /StopHandle::ensure\(ctx\).subscribe\(\)/,
    'worker must use application shutdown'
  );
  has(
    'worker',
    /MissedTickBehavior::Delay/,
    'worker must not accumulate missed ticks'
  );
  has(
    'worker',
    /runtime.process_media_releases\(100\)/,
    'worker must actually drain committed removals'
  );
  has(
    'operator',
    /PRODUCTS_MANAGE/,
    'operator HTTP backfill must enforce RBAC'
  );
  has(
    'operator',
    /admin_product_command_idempotency_key\(&headers\)/,
    'operator HTTP backfill must preserve caller retry identity'
  );
  has(
    'routes',
    /products\/media-references\/backfill/,
    'operator backfill must be reachable'
  );
  has(
    'routes',
    /products\/media-references\/audit/,
    'operator audit must be reachable'
  );
  has(
    'operator',
    /ensure_scope\(&tenant, &auth, &request\)/,
    'operator endpoints must reject substituted authenticated tenant context'
  );
  has(
    'context',
    /for permission in &auth.permissions/,
    'REST context must propagate authenticated permission claims'
  );
  has(
    'context',
    /request_context.correlation_id.clone\(\)/,
    'REST context must propagate request correlation'
  );
  has(
    'routes',
    /products\/media-references\/releases/,
    'bounded release status must be reachable'
  );
  has(
    'openapi',
    /crate::controllers::admin::product_media::backfill/,
    'operator workflow must be published in OpenAPI'
  );
  has(
    'reference',
    /if !report.legacy_images_remaining/,
    'tenant readiness must only seal after full legacy coverage'
  );
  has(
    'reference',
    /product_media_retention_readiness::Entity::find_by_id\(tenant_id\)/,
    'steady-state admission needs an indexed readiness point lookup'
  );
  has(
    'runtime',
    /runtime.media_maintenance_port = Some\(Arc::new\(catalog.clone\(\)\)\)/,
    'local queue inspection must not require a live Media capability'
  );
  has(
    'consumer',
    /"media.asset_read.v1"/,
    'consumer registry must declare exact owner read contract'
  );
  has(
    'consumer',
    /"media.asset_write.v1"/,
    'consumer registry must declare exact owner write contract'
  );
  has(
    'consumer',
    /source_implemented_runtime_pending/,
    'consumer metadata must not invent runtime certification'
  );
  has(
    'manifest',
    /profile = "product_image_retention"/,
    'manifest must publish the Media consumer profile'
  );
  absent('worker', /ctx.db_clone\(|shared_get::<Arc<dyn ProductMediaReferencePort>>/,
    'worker must not mix host persistence and a separately published Media authority');
  has('runtime', /catalog\s*\.process_media_releases\(batch_limit\)/,
    'delivery must use the command owner catalog snapshot');
  has('services', /#\[cfg\(feature = "mod-product"\)\]\s*pub mod product_media_release_worker;/,
    'neutral release delivery must not require the embedded Media module');
  has('dispatcher', /#\[cfg\(feature = "mod-product"\)\]\s*fn spawn_product_media_release_worker_if_enabled/,
    'worker lifecycle must be available without embedded Media');
  has('reference', /fn require_maintenance_permission[\s\S]*?PortError::forbidden\(/,
    'maintenance permission denial must stay Forbidden at the port boundary');
  for (const write of ['false', 'true']) {
    has('reference', new RegExp(`require_maintenance_permission\\(&context, ${write}\\)\\?`),
      'maintenance ports must enforce permission before storage IO');
  }
  has('operator', /PortErrorKind::Forbidden => HttpError::forbidden\(/,
    'operator must preserve owner Forbidden rather than report an outage');
  has('reference', /local_images.get\(&hold.image_id\)/,
    'hold audit must resolve its bounded Product page in a batch');
  has('reference', /Attempts.eq\(job.attempts\)/, 'stale failure must compare retry generation before updating');
  absent('reference', /model.update\(db\)/, 'concurrent acknowledgement must not abort remaining batch');
  has('integration', /native_owner_fences_gc_and_refuses_historical_hold_acknowledgements/, 'native Media owner integration source is required');
  has('integration', /loopback_owner_fences_gc_and_refuses_historical_hold_acknowledgements/, 'real loopback Media owner integration source is required');
  has('integration', /MediaGrpcService::new\(fixture.media.clone\(\)\)/, 'loopback tests must use the actual Media owner');
  return errors;
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const root =
    process.env.RUSTOK_VERIFY_REPO_ROOT ??
    path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
  const sources = Object.fromEntries(
    Object.entries(files).map(([key, file]) => [
      key,
      readFileSync(path.join(root, file), 'utf8')
    ])
  );
  const errors = verify(sources);
  if (errors.length) {
    console.error(errors.join('\n'));
    process.exitCode = 1;
  } else
    console.log(
      '[verify-product-media-retention] durable Media owner workflow source-locked (not runtime proof)'
    );
}
