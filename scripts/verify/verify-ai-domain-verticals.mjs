#!/usr/bin/env node
import { existsSync, readFileSync } from 'node:fs';

const failures = [];
function read(path) { return readFileSync(path, 'utf8'); }
function fail(message) { failures.push(message); }
function assertExists(path) { if (!existsSync(path)) fail(`${path}: missing required file`); }
function assertIncludes(text, needle, label) { if (!text.includes(needle)) fail(`${label}: missing ${needle}`); }
function assertNotIncludes(text, needle, label) { if (text.includes(needle)) fail(`${label}: forbidden ${needle}`); }
function assertOrdered(text, needles, label) {
  let cursor = -1;
  for (const needle of needles) {
    const next = text.indexOf(needle, cursor + 1);
    if (next === -1) fail(`${label}: missing ordered marker ${needle}`);
    if (next !== -1 && next < cursor) fail(`${label}: marker ${needle} appears out of order`);
    cursor = next;
  }
}

const files = {
  product: 'crates/rustok-ai-product/src/lib.rs',
  content: 'crates/rustok-ai-content/src/lib.rs',
  order: 'crates/rustok-ai-order/src/lib.rs',
  media: 'crates/rustok-ai-media/src/lib.rs',
  alloy: 'crates/rustok-ai-alloy/src/lib.rs',
  commerceBinding: 'crates/rustok-ai/src/direct_domain_commerce.rs',
  contentBinding: 'crates/rustok-ai/src/direct_domain_content.rs',
  serviceHelpers: 'crates/rustok-ai/src/service/helpers.rs',
  orderBinding: 'crates/rustok-ai/src/direct_domain_orders.rs',
  mediaBinding: 'crates/rustok-ai/src/direct_domain_media.rs',
  alloyBinding: 'crates/rustok-ai/src/direct_domain_alloy.rs',
  productPlan: 'crates/rustok-ai-product/docs/implementation-plan.md',
  contentPlan: 'crates/rustok-ai-content/docs/implementation-plan.md',
  orderPlan: 'crates/rustok-ai-order/docs/implementation-plan.md',
  mediaPlan: 'crates/rustok-ai-media/docs/implementation-plan.md',
  alloyPlan: 'crates/rustok-ai-alloy/docs/implementation-plan.md',
  aiPlan: 'crates/rustok-ai/docs/implementation-plan.md',
};

for (const file of Object.values(files)) assertExists(file);

const product = read(files.product);
const content = read(files.content);
const order = read(files.order);
const media = read(files.media);
const alloy = read(files.alloy);
const commerceBinding = read(files.commerceBinding);
const contentBinding = read(files.contentBinding);
const orderBinding = read(files.orderBinding);
const mediaBinding = read(files.mediaBinding);
const alloyBinding = read(files.alloyBinding);
const serviceHelpers = read(files.serviceHelpers);
const productPlan = read(files.productPlan);
const contentPlan = read(files.contentPlan);
const orderPlan = read(files.orderPlan);
const mediaPlan = read(files.mediaPlan);
const alloyPlan = read(files.alloyPlan);
const aiPlan = read(files.aiPlan);

assertOrdered(product, [
  'PRODUCT_COPY_TASK_SLUG: &str = "product_copy"',
  'PRODUCT_ATTRIBUTES_TASK_SLUG: &str = "product_attributes"',
  'pub struct ProductAiVerticalDescriptor',
  'pub const PRODUCT_AI_VERTICALS',
  'pub fn register_product_ai_vertical_handlers',
  'pub struct GeneratedProductCopy',
  'pub struct GeneratedProductAttributes',
  'pub fn validate_product_attributes_payload',
  'pub fn validate_product_copy_payload',
], 'rustok-ai-product source contract');
for (const marker of [
  'rejects_blank_product_attributes_flex_key_or_value',
  'accepts_product_copy_with_any_non_empty_field',
  'rejects_empty_product_copy',
]) assertIncludes(product, marker, 'rustok-ai-product contract tests');
assertIncludes(commerceBinding, 'register_product_ai_vertical_handlers', 'commerce runtime binding');
assertIncludes(commerceBinding, 'PRODUCT_COPY_TASK_SLUG', 'commerce runtime binding');
assertIncludes(commerceBinding, 'PRODUCT_ATTRIBUTES_TASK_SLUG', 'commerce runtime binding');
assertNotIncludes(commerceBinding, '"product_copy"', 'commerce runtime binding must not own product slugs');
assertNotIncludes(commerceBinding, '"product_attributes"', 'commerce runtime binding must not own product slugs');

assertOrdered(content, [
  'CONTENT_MODERATION_TASK_SLUG: &str = "content_moderation"',
  'BLOG_DRAFT_TASK_SLUG: &str = "blog_draft"',
  'pub enum ContentAiApprovalMode',
  'pub const CONTENT_AI_VERTICALS',
  'pub const CONTENT_AI_POLICY_MATRIX',
  'pub fn content_ai_sensitive_tools',
  'pub fn register_content_ai_vertical_handlers',
  'pub struct GeneratedBlogDraft',
  'pub fn validate_blog_draft_payload',
  'pub struct GeneratedModerationDecision',
  'pub fn validate_moderation_decision',
], 'rustok-ai-content source contract');
for (const marker of [
  'ContentAiApprovalMode::OperatorApproval',
  'ContentAiApprovalMode::Auto',
  'rejects_blank_blog_draft_fields_when_provided',
  'normalizes_known_decisions',
  'rejects_unknown_decisions',
]) assertIncludes(content, marker, 'rustok-ai-content contract tests/policy');
assertIncludes(contentBinding, 'register_content_ai_vertical_handlers', 'content runtime binding');
assertIncludes(serviceHelpers, 'content_ai_sensitive_tools', 'content runtime policy binding');
assertIncludes(serviceHelpers, 'merge_content_ai_sensitive_tools', 'content runtime policy binding');
assertNotIncludes(contentBinding, '"content_moderation"', 'content runtime binding must not own content slugs');
assertNotIncludes(contentBinding, '"blog_draft"', 'content runtime binding must not own content slugs');

assertOrdered(order, [
  'ORDER_ANALYTICS_TASK_SLUG: &str = "order_analytics"',
  'ORDER_OPS_ASSISTANT_TASK_SLUG: &str = "order_ops_assistant"',
  'pub struct OrderAiVerticalDescriptor',
  'pub const ORDER_AI_VERTICALS',
  'pub fn register_order_ai_vertical_handlers',
  'pub struct GeneratedOrderAnalytics',
  'pub struct GeneratedOrderOpsAssistant',
  'pub fn validate_order_analytics_payload',
  'pub fn validate_order_ops_assistant_payload',
], 'rustok-ai-order source contract');
for (const marker of [
  'rejects_blank_order_analytics_array_items',
  'rejects_empty_ops_fields',
  'rejects_null_ops_prefill',
]) assertIncludes(order, marker, 'rustok-ai-order contract tests');
assertIncludes(orderBinding, 'register_order_ai_vertical_handlers', 'order runtime binding');
assertIncludes(orderBinding, 'ORDER_ANALYTICS_TASK_SLUG', 'order runtime binding');
assertIncludes(orderBinding, 'ORDER_OPS_ASSISTANT_TASK_SLUG', 'order runtime binding');
assertNotIncludes(orderBinding, '"order_analytics"', 'order runtime binding must not own order slugs');
assertNotIncludes(orderBinding, '"order_ops_assistant"', 'order runtime binding must not own order slugs');


assertOrdered(media, [
  'IMAGE_ASSET_TASK_SLUG: &str = "image_asset"',
  'IMAGE_ASSET_TOOL_NAME: &str = "direct.media.generate_image"',
  'pub struct MediaAiVerticalDescriptor',
  'pub const MEDIA_AI_VERTICALS',
  'pub fn register_media_ai_vertical_handlers',
  'pub fn normalize_image_size',
], 'rustok-ai-media source contract');
for (const marker of [
  'test_normalize_image_size',
  'width == 0 || height == 0 || width > 4096 || height > 4096',
]) assertIncludes(media, marker, 'rustok-ai-media contract tests/validation');
assertIncludes(mediaBinding, 'register_media_ai_vertical_handlers', 'media runtime binding');
assertIncludes(mediaBinding, 'IMAGE_ASSET_TASK_SLUG', 'media runtime binding');
assertNotIncludes(mediaBinding, '"image_asset"', 'media runtime binding must not own media slugs');
assertIncludes(mediaPlan, 'ai-media-runtime-fallback-smoke.json', 'media plan fallback evidence');

assertOrdered(alloy, [
  'ALLOY_CODE_TASK_SLUG: &str = "alloy_code"',
  'ALLOY_CODE_TOOL_NAME: &str = "direct.alloy.run_script"',
  'pub struct AlloyAiVerticalDescriptor',
  'pub struct AlloyScriptExecutionPolicy',
  'pub const ALLOY_AI_VERTICALS',
  'pub const ALLOY_SCRIPT_ALLOWED_OPERATIONS',
  'pub const ALLOY_SCRIPT_EXECUTION_POLICY',
  'pub fn register_alloy_ai_vertical_handlers',
  'pub fn alloy_script_execution_policy',
  'pub fn validate_runtime_payload',
], 'rustok-ai-alloy source contract');
for (const marker of [
  'test_validate_runtime_payload',
  'test_alloy_descriptor_records_runtime_policy',
  'test_alloy_execution_policy_records_allowed_operations',
]) assertIncludes(alloy, marker, 'rustok-ai-alloy contract tests/policy');
assertIncludes(alloyBinding, 'register_alloy_ai_vertical_handlers', 'alloy runtime binding');
assertIncludes(alloyBinding, 'ALLOY_CODE_TASK_SLUG', 'alloy runtime binding');
assertNotIncludes(alloyBinding, '"alloy_code"', 'alloy runtime binding must not own alloy slugs');
assertIncludes(alloyPlan, 'ai-alloy-policy-registry.json', 'alloy plan policy evidence');

for (const [label, plan] of [['product plan', productPlan], ['content plan', contentPlan], ['order plan', orderPlan], ['media plan', mediaPlan], ['alloy plan', alloyPlan]]) {
  assertIncludes(plan, 'Execution checkpoint', label);
  assertIncludes(plan, 'rustok-ai', label);
}
assertIncludes(productPlan, 'compile-free static verification', 'product plan evidence');
assertIncludes(contentPlan, 'compile-free static verification', 'content plan evidence');
assertIncludes(orderPlan, 'compile-free static verification', 'order plan evidence');
assertIncludes(mediaPlan, 'static evidence', 'media plan evidence');
assertIncludes(alloyPlan, 'static evidence', 'alloy plan evidence');
assertIncludes(aiPlan, 'scripts/verify/verify-ai-domain-verticals.mjs', 'rustok-ai plan evidence');

if (failures.length > 0) {
  console.error('AI domain vertical static verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log('AI domain vertical static verification passed');
