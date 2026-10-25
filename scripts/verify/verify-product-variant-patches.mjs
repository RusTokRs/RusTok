#!/usr/bin/env node
// Source invariants only: not Rust/TypeScript compilation, tests, or runtime evidence.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root = process.env.RUSTOK_VERIFY_REPO_ROOT ?? path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = p => readFileSync(path.join(root, p), 'utf8');
const section = (s, start, end) => { const a = s.indexOf(start); if (a < 0) throw new Error(`Missing ${start}`); const z = s.indexOf(end, a + start.length); return s.slice(a, z < 0 ? undefined : z); };
export function run() {
  const errors = [];
  const require = (ok, label) => { if (!ok) errors.push(label); };
  const owner = 'crates/modules/rustok-product/';
  const commerce = 'crates/modules/rustok-commerce/src/graphql/';
  const fields = ['sku', 'barcode', 'shipping_profile_slug', 'weight', 'weight_unit'];
  const dto = section(read(`${owner}src/dto/variant.rs`), 'pub struct UpdateVariantInput', '\n#[derive');
  const command = section(read(`${owner}src/services/catalog/commands.rs`), 'pub async fn update_variant(', 'pub async fn ');
  const gql = section(read(`${commerce}types.rs`), 'pub struct UpdateVariantInput', '\n#[derive');
  const conversion = section(read(`${commerce}mutations/helpers.rs`), 'pub(crate) fn convert_update_variant_input(', '\npub(crate) fn ');
  const model = section(read(`${owner}admin/src/model.rs`), 'pub struct VariantUpdateDraft', '\nimpl VariantUpdateDraft');
  const wire = section(read(`${owner}admin/src/transport/product_lifecycle_graphql.rs`), 'struct UpdateVariantInput', '\n#[derive');
  const transport = section(read(`${owner}admin/src/transport/product_lifecycle_graphql.rs`), 'pub(crate) async fn update_product_variant(', 'pub(crate) async fn ');
  for (const field of fields) {
    require(new RegExp(`pub ${field}: Patch<`).test(dto), `owner ${field}: Patch required`);
    require(new RegExp(`skip_serializing_if = "Patch::is_keep"\\)\\]\\s*#\\[schema[^\\n]*\\]\\s*pub ${field}:`).test(dto), `owner ${field}: Keep must serialize as omission`);
    require(new RegExp(`pub ${field}: MaybeUndefined<String>`).test(gql), `GraphQL ${field}: omission/null must differ`);
    require(conversion.includes(`graphql_patch(input.${field})`), `GraphQL ${field}: preserve patch intent`);
    require(new RegExp(`match &?input\\.${field} \\{[\\s\\S]*?Patch::Keep => \\{\\},?[\\s\\S]*?Patch::Clear => active\\.${field} = Set\\(None\\)`).test(command), `owner ${field}: Keep must not assign, Clear must write NULL`);
    require(new RegExp(`skip_serializing_if = "rustok_api::Patch::is_keep"[\\s\\S]*?pub ${field}: rustok_api::Patch<String>`).test(model), `draft ${field}: preserve absent/null/value`);
    require(new RegExp(`skip_serializing_if = "rustok_api::Patch::is_keep"[\\s\\S]*?${field}: rustok_api::Patch<String>`).test(wire), `wire ${field}: never serialize Keep as null`);
    require(transport.includes(`${field}: draft.${field},`), `transport ${field}: do not collapse update intent`);
  }
  require(command.includes('if let Patch::Set(slug) = &input.shipping_profile_slug'), 'Shipping lookup must be Set-only');
  for (const field of ['sku', 'barcode', 'shipping_profile_slug', 'weight_unit']) {
    const validation = command.indexOf(`validate_patch_text_length(&input.${field},`);
    require(validation >= 0 && validation < command.indexOf('self.validate_shipping_profiles('), `${field}: length bound before provider IO`);
  }
  require(conversion.includes('Decimal::from_str_exact(&value)'), 'GraphQL weight must not round silently');
  require(!read('crates/libs/rustok-commerce-foundation/src/dto/variant.rs').includes('pub struct UpdateVariantInput'), 'Product must own the only canonical Variant update DTO');
  require(transport.includes('draft: crate::model::VariantUpdateDraft'), 'update must not accept the creation draft');
  const retry = read(`${owner}admin/src/transport/retry.rs`);
  require(retry.includes('draft={draft:?}') && retry.includes('variant_update_intent(&tenant_id, &user_id, &variant_id, &draft)'), 'retry must fingerprint the complete draft and scope');
  const next = read('apps/next-admin/packages/rustok-product/src/api/products.ts');
  const nextDto = section(next, 'export type UpdateVariantInput = {', '\n};');
  const nextAction = section(read('apps/next-admin/src/app/dashboard/product/actions.ts'), 'export async function updateVariantAction(', '\nexport async function ');
  for (const field of ['sku', 'barcode', 'shippingProfileSlug', 'weight', 'weightUnit']) {
    require(nextDto.includes(`${field}?: string | null;`), `Next ${field}: preserve nullable intent`);
    require(nextAction.includes(`${field}: variant.${field},`), `Next action ${field}: forward intent without filtering`);
  }
  for (const file of ['src/catalog_command_port.rs', 'admin/src/ui/product_editor.rs', 'admin/src/ui/leptos.rs']) {
    const source = read(owner + file);
    require(file.startsWith('admin/') ? source.includes('VariantUpdateDraft') : source.includes('self.update_variant(tenant_id, actor_id, variant_id, input)'), `${file}: retain owner update path`);
  }
  require(!read(`${owner}admin/src/core.rs`).includes('requires a nullable Variant patch'), 'remove obsolete UI clear rejection');
  if (errors.length) throw new Error(errors.join('\n'));
  console.log('Product nullable Variant patches: source invariants PASS (runtime gates open)');
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) run();
