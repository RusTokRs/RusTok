#!/usr/bin/env node
// Source-order guardrails, not Rust compilation or database execution evidence.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const paths = {
  commands: 'crates/modules/rustok-product/src/services/catalog/commands.rs',
  values:
    'crates/modules/rustok-product/src/services/catalog_schema_service/values.rs',
  dto: 'crates/modules/rustok-product/src/dto/product.rs',
  next: 'apps/next-admin/src/app/dashboard/product/actions.ts',
  leptos: 'crates/modules/rustok-product/admin/src/ui/leptos.rs',
  routedEditor: 'crates/modules/rustok-product/admin/src/ui/product_editor.rs'
};
function body(source, marker) {
  const start = source.indexOf(marker);
  if (start < 0) return '';
  const open = source.indexOf('{', start);
  let depth = 1;
  for (let i = open + 1; i < source.length; i++) {
    if (source[i] === '{') depth++;
    if (source[i] === '}' && --depth === 0) return source.slice(open + 1, i);
  }
  return '';
}
export function verifyAtomicSave(source) {
  const failures = [];
  const require = (ok, message) => {
    if (!ok) failures.push(message);
  };
  for (const method of ['create_product', 'update_product']) {
    const command = body(source.commands, `pub async fn ${method}(`);
    const write = command.indexOf('.apply_product_attribute_batches_in_tx(');
    const publish = command.indexOf(
      '.validate_product_publish_requirements_in('
    );
    const commit = command.lastIndexOf('txn.commit().await?');
    require(write >= 0 &&
      publish > write &&
      commit >
        publish, `${method}: all batches must precede final publication validation and commit`);
    require(command.includes(
      '&input.attribute_values'
    ), `${method}: canonical batches must reach the transaction writer`);
    require(!command.includes(
      'validate_new_product_publish_requirements_in('
    ), `${method}: category-only preflight must not reject initial values before insertion`);
  }
  for (const method of [
    'save_product_attribute_values',
    'clear_detached_product_attribute_values'
  ]) {
    require(body(source.values, `pub async fn ${method}(`).includes(
      'advance_product_value_revision_in_tx('
    ), `${method}: standalone values must invalidate older document drafts`);
  }
  const batches = body(
    source.values,
    'async fn apply_product_attribute_batches_in_tx('
  );
  const writer = body(
    source.values,
    'async fn apply_product_attribute_values_in_tx('
  );
  require(batches.includes(
    '.apply_product_attribute_values_in_tx('
  ), 'batch coordinator must reuse the same typed patch writer');
  require(batches.includes(
    'seen_locales.insert(locale.clone())'
  ), 'canonical duplicate locales must be refused');
  require(writer.includes(
    '!seen_nonlocalized.insert(patch.attribute_id)'
  ), 'cross-locale shared-value conflicts must be refused');
  const lock = writer.indexOf('lock_product_for_update_in_tx(');
  const form = writer.indexOf('Self::load_effective_form_for_category_in(');
  require(lock >= 0 &&
    form >
      lock, 'shared writer must lock the parent before reading the candidate category');
  for (const helper of [batches, writer]) {
    for (const forbidden of [
      'ProductWriteTransaction::begin(',
      'txn.commit(',
      'record_product_operation_result(',
      '.validate_product_publish_requirements_in('
    ]) {
      require(!helper.includes(
        forbidden
      ), `shared value writer must not own finalization: ${forbidden}`);
    }
  }
  require(body(source.dto, 'pub fn changes_document(').includes(
    '!self.attribute_values.is_empty()'
  ), 'value batches must require the document predecessor revision');
  require(source.next.includes(
    'attributeValues: payload.attributePatches.length'
  ), 'Next save must compose value batches');
  require(!source.next.includes(
    'await saveProductAttributeValues('
  ), 'Next save must not issue a second EAV command');
  require(!source.next.includes(
    'await deleteProductDetail(opts, created.id)'
  ), 'Next save must not delete a receipted create');
  const editor = source.leptos.slice(
    source.leptos.indexOf('let on_submit = move |ev: SubmitEvent|'),
    source.leptos.indexOf('let clear_detached_ui_locale')
  );
  require(editor.includes(
    'command.draft.attribute_values ='
  ), 'Leptos save must compose value batches');
  require(!editor.includes(
    'transport::save_product_attribute_values('
  ), 'Leptos save must not issue a second EAV command');
  require(source.routedEditor.includes(
    'attribute_values: if attribute_patches.is_empty()'
  ), 'mounted Leptos editor must compose value batches, including new products');
  require(source.routedEditor.includes(
    'effective_form=effective_form_resource'
  ), 'mounted Leptos fields must follow the candidate category');
  require(!source.routedEditor.includes(
    'let status_result ='
  ), 'mounted Leptos editor must not issue a second lifecycle command');
  require(!source.leptos.includes(
    'catalog_transport::save_product_attribute_values('
  ), 'mounted attribute fields must not own a separate save');
  const documentSave = source.routedEditor.slice(
    source.routedEditor.indexOf('let save_product ='),
    source.routedEditor.indexOf('let save_variant =')
  );
  const variantSave = source.routedEditor.slice(
    source.routedEditor.indexOf('let save_variant ='),
    source.routedEditor.indexOf('let page_title =')
  );
  require(variantSave.includes(
    'catalog_transport::update_product_variant('
  ), 'explicit Variant action must use the canonical command');
  require(!documentSave.includes(
    'catalog_transport::update_product_variant('
  ), 'document Save must not send a secondary Variant command');
  require(!documentSave.includes('set_refresh_nonce.update(') &&
    !variantSave.includes(
      'set_refresh_nonce.update('
    ), 'independent acknowledgements must not refetch over dirty fields');
  require(documentSave.includes(
    'set_loaded_product.set(Some(saved))'
  ), 'document acknowledgement must advance only its confirmed snapshot');
  const snapshot = variantSave.indexOf(
    'let draft = crate::model::VariantUpdateDraft'
  );
  const io = variantSave.indexOf('spawn_local(async move');
  require(snapshot >= 0 &&
    io > snapshot &&
    !variantSave
      .slice(io)
      .includes(
        '.get_untracked()'
      ), 'Variant controls must be snapshotted before IO');
  require(variantSave.includes('variant.id == target_id') &&
    !variantSave.includes(
      '.variants.first()'
    ), 'Variant command must use the displayed target identity');
  require(variantSave.includes(
    'validate_editor_variant_draft(&draft)'
  ), 'Variant scalar/nullable admission must precede transport');
  require(source.routedEditor.includes('on:click=save_variant') &&
    source.routedEditor.includes(
      'Save only this variant'
    ), 'Variant save must be a separate visible action');
  require(source.routedEditor.includes(
    'save_product_cb_active(if is_editing { None } else { Some("ACTIVE") })'
  ), 'existing document Save must respect selected status instead of implicitly publishing');
  require(variantSave.includes(
    'inventory_quantity_valid.get_untracked()'
  ), 'invalid inventory input must not send a stale previous quantity');
  return failures;
}
if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const source = Object.fromEntries(
    Object.entries(paths).map(([key, path]) => [
      key,
      readFileSync(resolve(path), 'utf8')
    ])
  );
  const failures = verifyAtomicSave(source);
  if (failures.length) {
    console.error(failures.join('\n'));
    process.exitCode = 1;
  } else
    console.log(
      'Product atomic editor save: owner transaction and both editors source-locked (not runtime evidence)'
    );
}
