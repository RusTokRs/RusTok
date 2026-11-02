#!/usr/bin/env node
/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

/**
 * Product axis/SKU source reachability guard (PROD-AXES-001).
 * Pins the canonical owner -> selected transport -> mounted editor chains and
 * immutable recovery sources. This source-only guard is NOT runtime evidence:
 * it cannot prove receipt, browser, transaction or macro behavior.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const product = 'crates/modules/rustok-product';
const admin = `${product}/admin/src`;
const next = 'apps/next-admin/packages/rustok-product/src';
const paths = {
  ownerAxes: `${product}/src/services/catalog/variant_axes.rs`,
  ownerPort: `${product}/src/catalog_command_port.rs`,
  commerce: 'crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs',
  journal: `${admin}/core/write_journal.rs`,
  browser: `${admin}/transport/write_journal.rs`,
  axes: `${admin}/core/variant_axes.rs`,
  create: `${admin}/core/variant_create.rs`,
  axesTransport: `${admin}/transport/variant_axes.rs`,
  createTransport: `${admin}/transport/variant_create.rs`,
  axesGraphql: `${admin}/transport/variant_axes_graphql.rs`,
  createGraphql: `${admin}/transport/variant_create_graphql.rs`,
  axesNative: `${admin}/transport/variant_axes_native.rs`,
  createNative: `${admin}/transport/variant_create_native.rs`,
  retry: `${admin}/transport/retry.rs`,
  lifecycle: `${admin}/transport/product_lifecycle_graphql.rs`,
  facade: `${admin}/catalog_transport.rs`,
  axesUi: `${admin}/ui/variant_axes.rs`,
  createUi: `${admin}/ui/variant_create.rs`,
  mounted: `${admin}/ui/product_editor.rs`,
  reference: `${admin}/ui/leptos.rs`,
  catalogReference: `${admin}/ui/catalog_admin.rs`,
  nextApi: `${next}/api/products.ts`,
  nextJournal: `${next}/write-journal.ts`,
  nextAttempt: `${next}/write-attempt.ts`,
  nextCreate: `${next}/variant-create.ts`,
  nextCard: `${next}/components/products/product-variant-axes-card.tsx`,
  nextPage: `${next}/pages/product-editor-page.tsx`,
  nextActions: 'apps/next-admin/src/app/dashboard/product/actions.ts',
  nextRoute: 'apps/next-admin/src/app/dashboard/product/[productId]/page.tsx'
};
const sources = Object.fromEntries(
  Object.entries(paths).map(([key, file]) => [
    key,
    readFileSync(path.join(root, file), 'utf8')
  ])
);
const failures = [];
const contains = (key, ...markers) => {
  for (const marker of markers) {
    if (!sources[key].includes(marker))
      failures.push(`${paths[key]}: missing ${marker}`);
  }
};
const absent = (key, ...markers) => {
  for (const marker of markers) {
    if (sources[key].includes(marker))
      failures.push(`${paths[key]}: forbidden ${marker}`);
  }
};

// One owner planner/application/receipt, not a UI-only or Commerce-owned writer.
contains(
  'ownerAxes',
  'pub async fn preview_variant_axes(',
  'pub async fn set_variant_axes(',
  'validate_variant_axis_configuration_in(',
  'lock_schema_for_product_write(',
  'expected != plan.preview.state_token',
  'txn.rollback().await?'
);
contains(
  'ownerPort',
  'async fn preview_variant_axes(',
  'async fn set_variant_axes(',
  'async fn create_variant(',
  'execute_catalog_command('
);
contains(
  'commerce',
  'async fn set_product_variant_axes(',
  'async fn create_product_variant(',
  'Permission::PRODUCTS_UPDATE',
  'product_command_context(',
  '"create_product_variant"'
);

// Axes/SKU and dictionary operations share the scoped journal. Associated scopes leave
// deployed Product JSON/prefixes intact instead of mislabeling an Attribute as a Product.
contains(
  'journal',
  'struct WriteJournal<S: JournalState>',
  'trait JournalScope:',
  'type Scope: JournalScope;',
  'pub scope: S::Scope',
  'fn can_transition_to(',
  'fn can_restore(',
  'fn fresh_from_live(',
  'self.scope.same_identity(',
  'axis_recovery',
  'variant_recovery'
);
contains(
  'browser',
  'session_storage()',
  'replace_state_with_url(',
  'can_transition_to(',
  'pub(crate) fn verify',
  'pub(crate) fn store'
);
contains(
  'axes',
  'WriteJournal<AxisJournalPhase>',
  'rustok.product.axis_attempt',
  'expected_state'
);
contains(
  'create',
  'WriteJournal<VariantCreatePhase>',
  'rustok.product.variant_create_attempt',
  'variant_id',
  'normalized_refusal(',
  'valid_input('
);

// Selection happens before IO. The native adapters invoke the selected runtime port.
for (const key of ['axesTransport', 'createTransport']) {
  contains(
    key,
    'cfg(any(feature = "ssr", feature = "hydrate"))',
    'authenticated_write_scope_matches('
  );
}
contains(
  'axesTransport',
  'preview_variant_axes_native(',
  'set_variant_axes_native('
);
contains(
  'createTransport',
  'create_variant_native(',
  'VariantCreateOutcome::Unknown'
);
contains(
  'axesGraphql',
  'previewProductVariantAxes(',
  'setProductVariantAxes(',
  'execute_with_metadata(',
  'definitive_refusal('
);
contains(
  'createGraphql',
  'createProductVariant(',
  'execute_with_metadata(',
  'definitive_refusal('
);
for (const key of ['axesNative', 'createNative']) {
  contains(
    key,
    'input = leptos::server_fn::codec::Json',
    'catalog_update_scope(',
    'write_context(',
    '.command_port()'
  );
}
contains('axesNative', '.preview_variant_axes(', '.set_variant_axes(');
contains(
  'createNative',
  '.create_variant(',
  '"create_product_variant"',
  'value.product_id == product_id'
);
absent(
  'retry',
  'async fn create_product_variant(',
  'ProductAdminLifecycleOperation::CreateVariant'
);
absent('lifecycle', 'async fn create_product_variant(');
contains(
  'facade',
  'transport::variant_axes::{preview_variant_axes, set_variant_axes}',
  'transport::variant_create::create_product_variant'
);

// Stable mounts live outside the document fieldset; controllers delegate core rules.
contains(
  'axesUi',
  'pub(super) fn ProductVariantAxesSection(',
  'before_begin: Callback<(), bool>',
  'write_journal::load::<AxisJournalPhase>',
  'catalog_transport::set_variant_axes(',
  'can_restore('
);
contains(
  'createUi',
  'pub(super) fn ProductVariantCreateSection(',
  'before_begin: Callback<(), bool>',
  'write_journal::load::<VariantCreatePhase>',
  'catalog_transport::create_product_variant(',
  'record.try_get_untracked()',
  'write_journal::store('
);
contains(
  'mounted',
  '<ProductVariantAxesSection',
  '<ProductVariantCreateSection',
  'let product_write_blocked',
  'write_journal::verify(record)'
);
absent('mounted', 'variant_axes_str', 'Comma-separated attribute axes');
contains(
  'reference',
  'fn ProductAdmin(',
  'product_admin_selected_product_query_state(query.get())',
  'selected_id.get().into_iter()',
  '<super::product_editor::ProductEditorPage'
);
contains('catalogReference', '<super::leptos::ProductAdmin');

// Next keeps caller identity/locale explicit; no fresh UUID can appear in SDK replay.
contains(
  'nextApi',
  'export const SET_VARIANT_AXES_MUTATION',
  'export async function setVariantAxes(',
  'export async function createVariant(',
  'idempotencyKey: string',
  'locale: opts.locale'
);
const createApi = sources.nextApi.slice(
  sources.nextApi.indexOf('export async function createVariant('),
  sources.nextApi.indexOf('export async function updateVariant(')
);
if (createApi.includes('randomUUID'))
  failures.push(`${paths.nextApi}: SKU replay regenerates the caller key`);
contains(
  'nextCreate',
  'isVariantCreateInput(',
  'prepareVariantCreateInput(',
  'isVariantCreateRefusalCode'
);
contains(
  'nextJournal',
  'expected: ProductJournalRecord | null',
  'PRODUCT_RECOVERY_UNAVAILABLE',
  'record: null'
);
contains(
  'nextAttempt',
  'ProductVariantCreateRequest',
  'variantId',
  "operation === 'variant'"
);
contains(
  'nextActions',
  'export async function setVariantAxesAction(',
  'export async function addVariantAction(',
  'getProductWriteAuthOpts(request.scope, idempotencyKey)',
  'request.input',
  'variantId: created.id'
);
contains(
  'nextCard',
  'export function ProductVariantAxesCard(',
  'allowedOptionIds',
  'onSaveAxes'
);
contains(
  'nextPage',
  '<ProductVariantAxesCard',
  'writer.resume(',
  'variant: onAddVariant',
  "writer.execute('variant'",
  'pendingVariantInput',
  'recoveryOnly'
);
contains(
  'nextRoute',
  'recoveryOnly',
  'onSetVariantAxes={setVariantAxesAction}',
  'onAddVariant={addVariantAction}'
);

if (failures.length > 0) {
  console.error('Product axis/SKU source reachability verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  '[verify-product-variant-axes-ui] Source reachability and immutable recovery markers are present; runtime remains unverified'
);
