#!/usr/bin/env node
// Source-contract checks only; compilation and database replay tests remain separate gates.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const sourcePaths = {
  port: 'crates/modules/rustok-product/src/catalog_command_port.rs',
  commands: 'crates/modules/rustok-product/src/services/catalog/commands.rs',
  projection:
    'crates/modules/rustok-product/src/services/catalog/projection.rs',
  tags: 'crates/modules/rustok-product/src/services/catalog/tags.rs',
  taxonomy: 'crates/modules/rustok-taxonomy/src/services.rs',
  transaction: 'crates/modules/rustok-product/src/services/write_transaction.rs'
};
const operations = [
  'create_product',
  'update_product',
  'delete_product',
  'publish_product',
  'unpublish_product',
  'archive_product',
  'create_variant',
  'update_variant',
  'delete_variant',
  'set_variant_axes',
  'add_product_image',
  'update_product_image',
  'delete_product_image',
  'reorder_product_images'
];

function method(source, signature) {
  const start = source.indexOf(signature);
  if (start < 0) return '';
  const rest = source.slice(start);
  const end = rest.indexOf('\n    }\n', signature.length);
  return end < 0 ? rest : rest.slice(0, end + '\n    }'.length);
}

export function verifyCatalogReceipts(sources) {
  const failures = [];
  const require = (condition, message) => {
    if (!condition) failures.push(message);
  };
  const implementation = sources.port.slice(
    sources.port.indexOf('impl ProductCatalogCommandPort for CatalogService')
  );
  for (const operation of operations) {
    const adapter = method(implementation, `    async fn ${operation}(`);
    require(adapter.includes(
      'execute_catalog_command('
    ), `${operation}: durable owner admission required`);
    require(adapter.includes('"actor": &context.actor') &&
      adapter.includes(
        '"locale": &context.locale'
      ), `${operation}: request must bind actor and locale`);
    const command = method(sources.commands, `    pub async fn ${operation}(`);
    require(command.length > 0, `${operation}: concrete command missing`);
    require(!/self\.get_(?:product(?:_with_locale_fallback)?|variant)\(/.test(
      command
    ), `${operation}: pool-based public response read forbidden`);
    let previousCommit = 0;
    for (const match of command.matchAll(/txn\.commit\(\)\.await\?;/g)) {
      const preceding = command.slice(previousCommit, match.index);
      require(preceding.includes(
        'record_product_operation_result('
      ), `${operation}: every commit, including no-op, must record its result`);
      previousCommit = match.index + match[0].length;
    }
    require(previousCommit > 0, `${operation}: transaction commit missing`);
    require(!command
      .slice(previousCommit)
      .includes(
        '.await'
      ), `${operation}: response must not require IO after final commit`);
  }
  const admission = sources.port.slice(
    sources.port.indexOf('async fn execute_catalog_command')
  );
  require(admission.indexOf('idempotency::admit(') >= 0 &&
    admission.indexOf('idempotency::admit(') <
      admission.indexOf(
        'with_product_operation_receipt(lease, future)'
      ), 'catalog receipt must admit before executing the owner future');
  for (const required of [
    'Admission::Replay(value)',
    'Admission::ReplayError(error)',
    'serde_json::from_value(value)',
    'fail_product_operation_receipt(service.database(), lease, &error)'
  ]) {
    require(admission.includes(
      required
    ), `catalog receipt: missing ${required}`);
  }
  require(!admission.includes(
    'idempotency::complete('
  ), 'catalog adapter must not complete success after commit');
  const productProjection = sources.projection.slice(
    sources.projection.indexOf('pub(super) async fn get_product_in'),
    sources.projection.indexOf('pub async fn get_variant(')
  );
  const variantProjection = sources.projection.slice(
    sources.projection.indexOf('pub(super) async fn get_variant_in'),
    sources.projection.indexOf('#[cfg(test)]')
  );
  require(productProjection.includes('self.load_product_tags_in(') &&
    !productProjection.includes(
      '&self.db'
    ), 'Product projection must use the supplied connection, including tags');
  require(variantProjection.includes('.one(conn)') &&
    !variantProjection.includes(
      '&self.db'
    ), 'Variant projection must use the supplied connection');
  const tagProjection = sources.tags.slice(
    sources.tags.indexOf('async fn load_product_tags_in'),
    sources.tags.indexOf('async fn sync_product_tags_in_tx')
  );
  require(tagProjection.includes('.all(conn)') &&
    tagProjection.includes('.resolve_term_names_in(conn,') &&
    !tagProjection.includes(
      '.all(&self.db)'
    ), 'tag projection must see uncommitted relations and Taxonomy names');
  const taxonomyProjection = method(
    sources.taxonomy,
    '    pub async fn resolve_term_names_in'
  );
  require(taxonomyProjection.includes('.all(conn)') &&
    taxonomyProjection.includes('load_translations_map_in(conn,') &&
    !taxonomyProjection.includes(
      '.all(&self.db)'
    ), 'Taxonomy name resolution must use the supplied connection');
  const completion = sources.transaction.slice(
    sources.transaction.indexOf('async fn complete_product_receipt_and_commit'),
    sources.transaction.indexOf('impl ProductWriteTransaction')
  );
  require(completion.includes('idempotency::complete(&transaction') &&
    completion.indexOf('idempotency::complete(&transaction') <
      completion.indexOf(
        'transaction.commit().await?'
      ), 'receipt success must be stored in the owner transaction before commit');
  return failures;
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const root = resolve(fileURLToPath(new URL('../..', import.meta.url)));
  const sources = Object.fromEntries(
    Object.entries(sourcePaths).map(([key, path]) => [
      key,
      readFileSync(resolve(root, path), 'utf8')
    ])
  );
  const failures = verifyCatalogReceipts(sources);
  if (failures.length) {
    console.error(failures.join('\n'));
    process.exitCode = 1;
  } else
    console.log(
      'Product catalog receipts: all 14 commands record pre-commit results; owner admission/replay/failure and transaction-aware projections are source-locked (not runtime evidence)'
    );
}
