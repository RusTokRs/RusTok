#!/usr/bin/env node
// Source guard only, not compilation, test execution, or durable-receipt evidence.
import {readFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root = process.env.RUSTOK_VERIFY_REPO_ROOT ?? path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = p => readFileSync(path.join(root, p), 'utf8');
function between(source, start, end) {
  const a = source.indexOf(start);
  if (a < 0) throw new Error(`Missing source marker: ${start}`);
  const z = source.indexOf(end, a + start.length);
  if (z < 0) throw new Error(`Missing source marker: ${end}`);
  return source.slice(a, z);
}
export function run() {
  const failures = [];
  const require = (ok, message) => { if (!ok) failures.push(message); };
  const product = 'crates/modules/rustok-product/src/';
  const adapter = between(read(product + 'error.rs'), 'impl From<rustok_core::field_schema::FlexError>', '/// Canonical classification of nested Core');
  const flex = between(read('crates/libs/rustok-core/src/field_schema.rs'), 'pub enum FlexError', 'impl std::fmt::Display for FlexError');
  const variants = [...flex.matchAll(/^    (\w+)(?:\(|\s*\{)/gm)].map(match => match[1]);
  require(variants.length > 0, 'Flex variant inventory must not be empty');
  for (const variant of variants) require(adapter.includes(`FlexError::${variant}`), `Flex ${variant} requires an explicit Product policy`);
  require(!/\b(?:other|_)\s*=>/.test(adapter), 'Flex conversion must remain exhaustive, without wildcard validation');
  require(!/format!|to_string\(/.test(adapter), 'Flex payload must never become error copy');
  require(/FlexError::Database\(message\)[\s\S]*?Self::Database\(sea_orm::DbErr::Custom\(message\)\)/.test(adapter), 'Flex database must preserve the storage class');
  require(/FlexError::UnknownEntityType\(_\)\s*=>\s*Self::Core\(CoreError::Internal/.test(adapter), 'Product fixed entity rejection must be internal, not validation');
  const errorSource = read(product + 'error.rs');
  const corePolicy = between(errorSource, 'fn core_error_policy(', 'pub fn core_port_error(');
  const coreEnum = between(read('crates/libs/rustok-core/src/error/mod.rs'), 'pub enum Error {', '// Conversion from old Error');
  const coreVariants = [...coreEnum.matchAll(/^    (\w+)(?:\(|\s*\{)/gm)].map(match => match[1]);
  require(coreVariants.length > 0, 'Core variant inventory must not be empty');
  for (const variant of coreVariants) require(corePolicy.includes(`CoreError::${variant}`), `Core ${variant}: explicit owner policy required`);
  require(!/\b(?:other|_)\s*=>/.test(corePolicy), 'Core classification must not have a catch-all');
  require(!/to_string\(|format!/.test(corePolicy), 'Core error copy must be static');
  const taxonomyPolicy = read(product + 'services/catalog_schema_service.rs');
  const syncPolicy = between(taxonomyPolicy, 'fn map_taxonomy_sync_error(', '/// A read projection');
  const readPolicy = between(taxonomyPolicy, 'fn map_taxonomy_read_error(', 'fn taxonomy_projection_error(');
  const taxonomyEnum = between(read('crates/modules/rustok-taxonomy/src/error.rs'), 'pub enum TaxonomyError', 'pub type TaxonomyResult');
  const taxonomyVariants = [...taxonomyEnum.matchAll(/^    (\w+)(?:\(|\s*\{)/gm)].map(match => match[1]);
  require(taxonomyVariants.length > 0, 'Taxonomy variant inventory must not be empty');
  for(const [policy,label] of [[syncPolicy,'sync'],[readPolicy,'read']]) {
    for(const variant of taxonomyVariants) require(policy.includes(`TaxonomyError::${variant}`), `Taxonomy ${label} ${variant}: explicit classification required`);
    require(!/\b(?:other|_)\s*=>|format!|to_string\(/.test(policy), `Taxonomy ${label}: no wildcard or payload copy`);
  }
  require(syncPolicy.includes('CommerceError::Conflict('), 'Taxonomy sync conflicts must not become validation');
  require(readPolicy.includes('taxonomy_projection_error(') && !readPolicy.includes('CommerceError::Validation('), 'Read projection failure must not be client validation');
  const mapperPaths = [
    'catalog_command_port.rs','catalog_schema_write_port.rs','catalog_schema_read_port.rs','storefront_tag_read_port.rs','media_reference.rs','ports/diagnostics.rs',
    'translation_target.rs','variant_translation_target.rs','variant_translation_progress_target.rs',
    'image_translation_target.rs','image_translation_progress_target.rs',
    ...['attributes/translation.rs','categories/translation.rs','categories/seo_translation.rs','schemas/translation.rs','values/translation.rs','values/variant_translation.rs'].map(p=>'services/catalog_schema_service/'+p),
  ];
  for(const file of mapperPaths) {
    const source=read(product+file);
    require(source.includes('CommerceError::Core(error) => crate::error::core_port_error(&error)'), `${file}: canonical Core mapping required`);
    require(source.includes('CommerceError::Conflict(_) => PortError::conflict('), `${file}: preserve typed conflict`);
  }
  require(read(product+'storefront_http_read_port.rs').includes('Some(crate::error::core_port_error(error))'), 'Storefront HTTP read capability must classify nested Core');
  const publicMapper=read(product+'public_error.rs');
  require(publicMapper.includes('CommerceError::Core(error) => core_public_error_policy(error)'), 'Native/shared GraphQL must classify nested Core');
  require(publicMapper.includes('crate::error::core_port_error(error)'), 'Public mapper must delegate canonical Core policy');
  const http=read('crates/modules/rustok-commerce/src/controllers/products.rs');
  require(http.includes('rustok_product::error::core_port_error(error).kind') && http.includes('product_core_http_policy(error, "commerce_admin_product_failed")'), 'HTTP admin must preserve nested Core kind and bounded copy');
  require(read('crates/modules/rustok-commerce/src/controllers/store/products.rs').includes('crate::controllers::products::product_core_http_policy('), 'HTTP storefront must share Core policy');
  const graphql=read('crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs');
  const outcome=between(graphql, 'fn product_command_error_outcome(', 'fn product_command_port_error(');
  require(outcome.includes('error.kind') && outcome.includes('"product.conflict"') && outcome.includes('"product.dependency_not_found"'), 'GraphQL definitive outcome must bind known domain classes and codes');
  const transaction = read(product + 'services/write_transaction.rs');
  const terminal = between(transaction, 'pub(crate) fn is_terminal_product_failure', '/// Persists only');
  require(terminal.includes('!error.retryable'), 'Retryable failures cannot finalize a receipt');
  const allowed = [...terminal.matchAll(/PortErrorKind::(\w+)/g)].map(match => match[1]).sort();
  require(allowed.join(',') === 'Conflict,NotFound,Validation', 'Terminal classes must be exactly Validation/NotFound/Conflict');
  const fail = between(transaction, 'pub(crate) async fn fail_product_operation_receipt', '/// The receipt operation UUID');
  require(/if !is_terminal_product_failure\(error\)[\s\S]*return Ok\(\(\)\);[\s\S]*idempotency::fail/.test(fail), 'Shared finalizer must gate the ledger write with the domain policy');
  const schema = read(product + 'catalog_schema_write_port.rs');
  require(schema.includes('if is_terminal_product_failure(&port_error)'), 'Schema finalization must use the same domain policy');
  require(read(product + 'services/catalog_schema_service/attributes.rs').includes('fail_product_operation_receipt(&self.db, lease, error).await'), 'Schema service must delegate to the shared finalizer');
  const command = read(product + 'catalog_command_port.rs');
  const mapper = between(command, 'fn product_command_error(', 'fn product_error_kind(');
  require(!/error_details|%error\b|\?error\b|error\.to_string\(/.test(mapper), 'Command error diagnostics must not render raw owner errors');
  for (const [source, label] of [[command, 'catalog'], [schema, 'schema']]) {
    const decode = label === 'catalog'
      ? between(source, 'idempotency::Admission::Replay(value)', 'idempotency::Admission::ReplayError')
      : between(source, 'fn decode_schema_receipt', 'fn schema_receipt_error');
    require(!/%error\b|\?error\b|error\.to_string\(/.test(decode), `${label} receipt decode must not log response-derived text`);
  }
  // Catalog command diagnostics must not serialize caller/receipt/owner payloads.
  const commandDiagnostics = command.split('#[cfg(test)]')[0];
  for (const marker of [
    'use crate::ports::{product_owner_error_facts, product_port_context_facts};',
    'let error_facts = product_owner_error_facts(&error);',
    'error_variant = error_facts.error_variant',
    'text_total_length = error_facts.text_total_length',
    'opaque_payload_present = error_facts.opaque_payload_present',
    'correlation_id_length = context_facts.correlation_id_length',
    'tenant_id_length = context_facts.tenant_id_length',
    'product_id_non_nil = !product_id.is_nil()',
    'actor_id_length = context_facts.actor_id_length',
    'owner_code_length = error.code.chars().count()',
    'owner_message_length = error.message.chars().count()',
  ]) require(commandDiagnostics.includes(marker), `catalog command diagnostic missing ${marker}`);
  require(!/internal_code\s*=|(?:correlation_id|tenant_id|product_id|actor_id)\s*=\s*[%?]|error\s*=\s*[%?]error|error\.to_string\(/.test(commandDiagnostics), 'Catalog command events must not render caller, identity, receipt or owner payload');
  for (const event of commandDiagnostics.matchAll(/tracing::(?:warn|error)!\(([\s\S]*?)\n\s*\);/g)) {
    const fields = event[1].replace(/error\.(?:code|message)\.chars\(\)\.count\(\)/g, 'bounded_length');
    require(!/\bcontext\.|\berror\.(?:code|message)\b/.test(fields), 'Command event must log projected context and only lengths of port payload strings');
    require(fields.includes('context_facts.'), 'Command event must consume its canonical context facts');
  }
  const commandEvents = commandDiagnostics.match(/tracing::(?:warn|error)!\(/g)?.length ?? 0;
  const commandFacts = commandDiagnostics.match(/let context_facts = product_port_context_facts\(/g)?.length ?? 0;
  require(commandEvents === 7 && commandFacts === commandEvents, 'Every catalog command event must use canonical bounded context facts');
  // Schema/tag read logs must share the owner fact projection, not serialize payloads.
  for (const file of ['catalog_schema_read_port.rs', 'storefront_tag_read_port.rs']) {
    const source = read(product + file).split('#[cfg(test)]')[0];
    require(source.includes('use crate::ports::{product_owner_error_facts, product_port_context_facts};'), `${file}: reuse canonical owner facts`);
    require(source.includes('let error_facts = product_owner_error_facts(&error);'), `${file}: bounded owner failure projection`);
    for (const marker of ['error_variant = error_facts.error_variant', 'text_total_length = error_facts.text_total_length', 'opaque_payload_present = error_facts.opaque_payload_present', 'correlation_id_length = context_facts.correlation_id_length', 'tenant_id_length = context_facts.tenant_id_length', 'owner_message_length = error.message.chars().count()']) {
      require(source.includes(marker), `${file}: missing bounded diagnostic ${marker}`);
    }
    require(!/\?error\b(?!\.kind\b)|%error\b|internal_message\s*=|internal_code\s*=|tenant_id\s*=\s*%|correlation_id\s*=\s*%|internal_attribute_id\s*=|error\.to_string\(/.test(source), `${file}: raw owner/context payload in diagnostics`);
    const events = source.match(/tracing::(?:error|warn)!\(/g)?.length ?? 0;
    const projections = source.match(/let context_facts = product_port_context_facts\(/g)?.length ?? 0;
    require(events === projections && events > 0, `${file}: each event needs bounded context facts`);
  }
  const httpRead = read(product + 'storefront_http_read_port.rs').split('#[cfg(test)]')[0];
  for (const marker of ['let context_facts = product_port_context_facts(context);',
    'let error_facts = product_owner_error_facts(&error);',
    'correlation_id_length = context_facts.correlation_id_length',
    'tenant_id_length = context_facts.tenant_id_length',
    'owner_error_variant = error_facts.error_variant',
    'opaque_payload_present = error_facts.opaque_payload_present',
    'page_zero = request.page == 0', 'per_page_zero = request.per_page == 0',
    'per_page_exceeds_limit = request.per_page > MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE']) {
    require(httpRead.replace(/\s/g, '').includes(marker.replace(/\s/g, '')), `HTTP read diagnostic missing ${marker}`);
  }
  require(!/correlation_id\s*=\s*%context|tenant_id\s*=\s*%context|\bpage\s*=\s*request.page,|\bper_page\s*=\s*request.per_page,|error\s*=\s*[?%]error/.test(httpRead), 'HTTP read diagnostics must not emit caller identity, error payload or exact rejected pagination');
  if (failures.length) throw new Error(failures.join('\n'));
  console.log('Product Flex/Taxonomy/Core/receipt boundary: source invariants PASS (runtime gates open)');
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) run();
