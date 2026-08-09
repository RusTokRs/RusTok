#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const failures = [];
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

const eventPath = 'crates/rustok-events/src/product_index_refresh.rs';
const contractPath = 'crates/rustok-events/src/contract.rs';
const eventsLibPath = 'crates/rustok-events/src/lib.rs';
const productBindingPath = 'crates/rustok-product/src/services/index_refresh_event.rs';
const productRelayPath = 'crates/rustok-product/src/services/index_refresh_relay.rs';
const productWriterPath = 'crates/rustok-product/src/services/index_refresh_publication.rs';
const productModPath = 'crates/rustok-product/src/services/mod.rs';
const digestPath = 'crates/rustok-events/contracts/event-contract-digests.json';
const docsPath = 'crates/rustok-product/docs/index-refresh-event-family.md';

for (const relative of [
  eventPath,
  contractPath,
  eventsLibPath,
  productBindingPath,
  productRelayPath,
  productWriterPath,
  productModPath,
  digestPath,
  docsPath,
]) {
  if (!fs.existsSync(path.join(root, relative))) failures.push(`missing ${relative}`);
}

if (failures.length > 0) {
  console.error('[verify-index-product-refresh-event-family] FAIL');
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const event = read(eventPath);
for (const marker of [
  'pub enum ProductIndexRefreshEvent',
  'LocaleRefreshRequested',
  'VariantRefreshRequested',
  '"product.index.locale_refresh_requested"',
  '"product.index.variant_refresh_requested"',
  'product_id: Uuid',
  'locale: String',
  'variant_id: Uuid',
  'source_version: u64',
  'PRODUCT_INDEX_REFRESH_EVENT_SCHEMA_VERSION: u16 = 1',
  'ContractEventPayload::ProductIndexRefresh(self)',
  'impl ValidateEvent for ProductIndexRefreshEvent',
]) need(event, marker, 'Product Index event family');
for (const forbidden of [
  'refresh_id:',
  'root_event_id:',
  'tenant_id:',
  'actor_id:',
  'causation_id:',
  'legacy',
  'V2',
  'v2',
]) forbid(event, forbidden, 'Product Index event payload');

const contract = read(contractPath);
for (const marker of [
  'ProductIndexRefreshEvent',
  '#[serde(rename = "product_index_refresh")]',
  'ProductIndexRefresh(ProductIndexRefreshEvent)',
  'Self::ProductIndexRefresh(event) => event.event_type()',
  'Self::ProductIndexRefresh(event) => event.schema_version()',
  'Self::ProductIndexRefresh(event) => event.validate()',
]) need(contract, marker, 'closed contract payload');

const eventsLib = read(eventsLibPath);
for (const marker of [
  'mod product_index_refresh;',
  'ProductIndexRefreshEvent',
  'product_index_refresh_event_schema(event_type)',
  '.chain(PRODUCT_INDEX_REFRESH_EVENT_SCHEMAS.iter())',
]) need(eventsLib, marker, 'event registry');

const productBinding = read(productBindingPath);
for (const marker of [
  'impl ProductIndexRefreshContract for ProductIndexRefreshEvent',
  'ProductIndexRefreshContractTarget::Locale',
  'ProductIndexRefreshContractTarget::Variant',
  'pub struct CanonicalProductIndexRefreshEventFactory;',
  'impl ProductIndexRefreshEventFactory for CanonicalProductIndexRefreshEventFactory',
  'record.product_id()',
  'record.locale().to_owned()',
  'record.variant_id()',
  'record.source_version()',
]) need(productBinding, marker, 'Product owner event binding');
for (const forbidden of [
  'Uuid::new_v4()',
  'root_event_id()',
  'refresh_id()',
]) forbid(productBinding, forbidden, 'Product event factory identity');

const productRelay = read(productRelayPath);
for (const marker of [
  'type LocaleEvent: ProductIndexRefreshContract;',
  'type VariantEvent: ProductIndexRefreshContract;',
  'self.factory.locale_event(&record)',
  'self.factory.variant_event(&record)',
  'ProductIndexRefreshCanonicalWriter::publish_locale_once_in_transaction',
  'ProductIndexRefreshCanonicalWriter::publish_variant_once_in_transaction',
]) need(productRelay, marker, 'durable relay binding');

const productWriter = read(productWriterPath);
for (const marker of [
  'record.refresh_id()',
  'record.root_event_id()',
  'publish_contract_once_direct_in_tx_with_envelope_id_and_causation',
  'ProductIndexRefreshContractTarget::Locale',
  'ProductIndexRefreshContractTarget::Variant',
]) need(productWriter, marker, 'canonical Product writer');

need(
  read(productModPath),
  'pub use index_refresh_event::CanonicalProductIndexRefreshEventFactory;',
  'Product service export',
);

const admittedBaseline = [
  'sha256:c11d7934e114d21b8a42eb471a53a87dbb85fa754e6b9443f0f85a0c03082c14',
  'sha256:4fb7217649a216bc8ba6e4723d311e9d1a5f81c6fc101363248825fe3f78b6f8',
  'sha256:be61a52aef58365f2b702ed808c01e97ff6a8752494c949e01b40c057e8d2725',
];
const digest = read(digestPath);
if (admittedBaseline.every((marker) => digest.includes(marker))) {
  failures.push(
    'event-contract digest still equals the pre-Product admitted baseline; run the canonical generator on this exact branch',
  );
}

const docs = read(docsPath);
for (const marker of [
  'Status: `source_ready_digest_regeneration_pending`.',
  'product.index.locale_refresh_requested',
  'product.index.variant_refresh_requested',
  'id = correlation_id = refresh_id',
  'causation_id = root_event_id',
  'cargo run --locked -p rustok-events --example event_contract_digests -- --write',
]) need(docs, marker, 'Product Index event documentation');

if (failures.length > 0) {
  console.error('[verify-index-product-refresh-event-family] FAIL');
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log('[verify-index-product-refresh-event-family] PASS typed_family=true canonical_factory=true digest_regenerated=true');
