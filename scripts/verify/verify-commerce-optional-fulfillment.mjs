#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const commerceModule = read('crates/modules/rustok-commerce/src/lib.rs');
const commerceManifest = read('crates/modules/rustok-commerce/rustok-module.toml');
const rootManifest = read('modules.toml');
const serverCargo = read('apps/server/Cargo.toml');

requireText(commerceModule, 'if let Some(fulfillment_registry) =', 'optional Fulfillment listener');
requireText(commerceModule, 'ctx.extensions.get::<FulfillmentProviderRegistry>().cloned()', 'Fulfillment capability lookup');
forbidText(commerceModule, '"fulfillment",\n        ]', 'CommerceModule module dependency');
forbidText(commerceModule, 'requires FulfillmentProviderRegistry in ModuleRuntimeExtensions', 'Commerce startup panic');
forbidText(commerceManifest, 'fulfillment = { version_req = ">=0.1.0" }', 'Commerce package dependency');
forbidText(rootManifest, 'depends_on = ["tenant", "cart", "customer", "product", "region", "pricing", "inventory", "order", "payment", "fulfillment"]', 'root Commerce dependency');
forbidText(serverCargo, 'mod-payment", "mod-fulfillment", "rustok-distribution/mod-commerce"', 'server Commerce feature dependency');

if (failures.length) {
  for (const failure of failures) console.error(failure);
  process.exit(1);
}
console.log('commerce optional Fulfillment composition boundary: ok');
