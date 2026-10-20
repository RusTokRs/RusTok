import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { paths, verifyAtomicSave } from './verify-product-atomic-save.mjs';

const canonical = Object.fromEntries(
  Object.entries(paths).map(([key, path]) => [key, readFileSync(path, 'utf8')])
);
test('current owner and both editors compose an atomic catalog save', () => {
  assert.deepEqual(verifyAtomicSave(canonical), []);
});
for (const [name, key, before, after, diagnostic] of [
  [
    'create preflight before initial values',
    'commands',
    'let schema = ProductCatalogSchemaService::new(self.db.clone(), self.event_bus.clone());',
    'schema.validate_product_publish_requirements_in(&txn, tenant_id, product_id).await?;\nlet schema = ProductCatalogSchemaService::new(self.db.clone(), self.event_bus.clone());',
    /precede final publication/
  ],
  [
    'category-only create gate',
    'commands',
    'if input.publish {',
    'if input.publish { validate_new_product_publish_requirements_in();',
    /category-only preflight/
  ],
  [
    'shared writer without parent lock',
    'values',
    'let product = lock_product_for_update_in_tx(txn, tenant_id, product_id).await?;',
    'let product = load_product_from_pool();',
    /lock the parent/
  ],
  [
    'batch coordinator commits independently',
    'values',
    'let mut seen_locales = HashSet::new();',
    'txn.commit().await?; let mut seen_locales = HashSet::new();',
    /must not own finalization/
  ],
  [
    'duplicate locales silently accepted',
    'values',
    'seen_locales.insert(locale.clone())',
    'true',
    /duplicate locales/
  ],
  [
    'shared nonlocalized last-write-wins',
    'values',
    '!seen_nonlocalized.insert(patch.attribute_id)',
    'false',
    /shared-value conflicts/
  ],
  [
    'value-only update without predecessor',
    'dto',
    '!self.attribute_values.is_empty()',
    'false',
    /predecessor revision/
  ],
  [
    'Next second EAV command',
    'next',
    'const opts = await getAuthOpts();',
    'await saveProductAttributeValues(); const opts = await getAuthOpts();',
    /second EAV/
  ],
  [
    'Next compensating delete',
    'next',
    'return { id: created.id, revision: created.revision };',
    'await deleteProductDetail(opts, created.id); return { id: created.id, revision: created.revision };',
    /receipted create/
  ],
  [
    'mounted Leptos defaults discard values',
    'routedEditor',
    'attribute_values: if attribute_patches.is_empty()',
    'attribute_values: if true',
    /mounted Leptos editor must compose/
  ],
  [
    'document silently submits Variant',
    'routedEditor',
    'let res = catalog_transport::update_product(',
    'catalog_transport::update_product_variant(); let res = catalog_transport::update_product(',
    /secondary Variant/
  ],
  [
    'document acknowledgement clobbers dirty controls',
    'routedEditor',
    'set_loaded_product.set(Some(saved));',
    'set_refresh_nonce.update(|n| *n += 1); set_loaded_product.set(Some(saved));',
    /refetch over dirty/
  ],
  [
    'Variant acknowledgement clobbers document draft',
    'routedEditor',
    'set_loaded_product.update(|product| {',
    'set_refresh_nonce.update(|n| *n += 1); set_loaded_product.update(|product| {',
    /refetch over dirty/
  ],
  [
    'Variant controls read after network starts',
    'routedEditor',
    'let result = catalog_transport::update_product_variant(',
    'sku.get_untracked(); let result = catalog_transport::update_product_variant(',
    /snapshotted before IO/
  ],
  [
    'Variant silently retargets first row',
    'routedEditor',
    'variant.id == target_id',
    'variant.id == target_id && product.variants.first().is_some()',
    /displayed target identity/
  ],
  [
    'Variant scalar admission removed',
    'routedEditor',
    'validate_editor_variant_draft(&draft)',
    'Ok::<(), String>(())',
    /scalar\/nullable admission/
  ],
  [
    'Variant has no independent visible control',
    'routedEditor',
    'on:click=save_variant',
    'on:click=move |_| {}',
    /separate visible action/
  ],
  [
    'document acknowledgement drops its new predecessor',
    'routedEditor',
    'set_loaded_product.set(Some(saved));',
    'drop(saved);',
    /confirmed snapshot/
  ],
  [
    'ordinary edit implicitly publishes',
    'routedEditor',
    'save_product_cb_active(if is_editing { None } else { Some("ACTIVE") })',
    'save_product_cb_active(Some("ACTIVE"))',
    /respect selected status/
  ],
  [
    'invalid inventory sends the old parsed number',
    'routedEditor',
    'if !inventory_quantity_valid.get_untracked() {',
    'if false {',
    /invalid inventory input/
  ],
  [
    'Leptos second EAV command',
    'leptos',
    'command.draft.attribute_values =',
    'transport::save_product_attribute_values(); command.draft.attribute_values =',
    /second EAV/
  ]
]) {
  test(`rejects ${name}`, () => {
    assert.ok(canonical[key].includes(before), 'mutation anchor must exist');
    const failures = verifyAtomicSave({
      ...canonical,
      [key]: canonical[key].replace(before, after)
    });
    assert.ok(
      failures.some((failure) => diagnostic.test(failure)),
      failures.join('\n')
    );
  });
}
