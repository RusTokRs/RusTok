#!/usr/bin/env node
// Source invariants only. Does not establish database/runtime/replay execution.
import {readFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
export function run() {
  const root = process.env.RUSTOK_VERIFY_REPO_ROOT ?? path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
  const read = p => readFileSync(path.join(root, p), 'utf8');
  const command = read('crates/modules/rustok-product/src/catalog_command_port.rs');
  const writes = read('crates/modules/rustok-product/src/services/catalog/commands.rs');
  const policy = read('crates/modules/rustok-product/src/shipping_profile.rs');
  const runtime = read('crates/modules/rustok-product/src/runtime.rs');
  const provider = read('crates/modules/rustok-commerce/src/services/shipping_profile.rs');
  const http = read('crates/modules/rustok-commerce/src/controllers/admin/products.rs');
  const graphql = read('crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs');
  const host = read('apps/server/src/services/commerce_provider_runtime.rs');
  const errors = [];
  const check = (source, re, label) => {if (!re.test(source)) errors.push(label);};
  const impl = command.slice(command.indexOf('impl ProductCatalogCommandPort for CatalogService'));
  for(const name of ['create_product','update_product','create_variant','update_variant']) {
    const start = impl.indexOf(`async fn ${name}(`);
    const end = impl.indexOf('\n    async fn ', start + 1);
    const method = impl.slice(start, end < 0 ? undefined : end);
    check(method, new RegExp(`execute_catalog_command\\([\\s\\S]*self\\.${name}\\(`),
      `${name}: writes must remain inside the lazy receipted future`);
    const writeStart = writes.indexOf(`pub async fn ${name}(`);
    const writeEnd = writes.indexOf('pub async fn ', writeStart + 1);
    const write = writes.slice(writeStart, writeEnd < 0 ? undefined : writeEnd);
    const policyIndex = write.indexOf('self.validate_shipping_profiles(');
    const transactionIndex = write.indexOf('ProductWriteTransaction::begin');
    if(policyIndex < 0 || transactionIndex < policyIndex) errors.push(`${name}: owner must validate shipping before transaction`);
  }
  check(command, /Admission::Replay\(value\)[\s\S]*return serde_json::from_value[\s\S]*with_product_operation_receipt\(lease, future\)/,
    'completed replay must precede policy future polling');
  check(writes, /input\s*\.variants\s*\.iter\(\)[\s\S]*variant.shipping_profile_slug.as_deref\(\)/,
    'Product creation must also validate initial Variant overrides');
  check(policy, /slugs.is_empty\(\)[\s\S]*return Ok\(\(\)\)/, 'empty selection must not require provider');
  check(policy, /shipping_profile_port\s*\.as_ref\(\)\s*\.ok_or_else/, 'nonempty selection must fail closed without capability');
  check(policy, /tokio::time::timeout/, 'policy IO must be bounded');
  check(policy, /PortErrorKind::NotFound/, 'only authoritative missing profile may terminally reject');
  check(policy, /!error.retryable/, 'retryable NotFound must not finalize rejection');
  const providerAdmission = provider.slice(provider.indexOf('impl rustok_product::ProductShippingProfilePort'));
  if (/error.to_string\(|format!\(/.test(providerAdmission)) errors.push('shipping provider errors must not expose owner payloads');
  if (/error.to_string\(|tracing::/.test(policy)) errors.push('policy must not expose or log raw provider errors');

  if (/rustok_commerce|shipping_profile::Entity/.test(policy)) errors.push('Product must not import foreign owner/storage');
  check(runtime, /with_shipping_profile_port[\s\S]*embedded_catalog[\s\S]*self.command_port = Arc::new\(catalog.clone\(\)\)/,
    'policy must be installed inside the embedded owner, not a proxy decorator');
  check(provider, /impl rustok_product::ProductShippingProfilePort for ShippingProfileService/, 'Commerce must implement neutral consumer port');
  check(provider, /ensure_shipping_profile_slug_exists\(tenant, &slug\)/, 'first execution must validate active owner profiles');
  check(host, /ProductCatalogCommandProfile::EmbeddedNative[\s\S]*shipping_profile_port\(\).is_none\(\)/,
    'host must preserve preconfigured and external owners');
  check(host, /with_shipping_profile_port\(port\)/, 'host must compose owner policy');
  for(const [source,label] of [[http,'HTTP'],[graphql,'GraphQL']]) {
    if (/validate_(admin_)?product_shipping_profile_input\(/.test(source)) errors.push(`${label} must not query mutable shipping state before replay`);
    check(source, /Permission::PRODUCTS_CREATE/, `${label} must retain create authorization`);
    check(source, /Permission::PRODUCTS_UPDATE/, `${label} must retain update authorization`);
    check(source, /command_port\(\)/, `${label} must use selected owner port`);
  }
  for(const file of ['crates/modules/rustok-commerce/src/controllers/mod.rs','crates/modules/rustok-commerce/src/graphql_runtime.rs']) {
    check(read(file), /with_shipping_profile_port\(/, `${file}: embedded fallback must install policy`);
  }
  if(errors.length) throw new Error(errors.join('\n'));
  console.log('Product Shipping Profile admission source-locked behind owner receipts (not runtime evidence)');
}
if(process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) run();
