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
  commerceBinding: 'crates/rustok-ai/src/direct_domain_commerce.rs',
  contentBinding: 'crates/rustok-ai/src/direct_domain_content.rs',
  serviceHelpers: 'crates/rustok-ai/src/service/helpers.rs',
  orderBinding: 'crates/rustok-ai/src/direct_domain_orders.rs',
  productPlan: 'crates/rustok-ai-product/docs/implementation-plan.md',
  contentPlan: 'crates/rustok-ai-content/docs/implementation-plan.md',
  orderPlan: 'crates/rustok-ai-order/docs/implementation-plan.md',
  aiPlan: 'crates/rustok-ai/docs/implementation-plan.md',
};

for (const file of Object.values(files)) assertExists(file);

const product = read(files.product);
const content = read(files.content);
const order = read(files.order);
const commerceBinding = read(files.commerceBinding);
const contentBinding = read(files.contentBinding);
const orderBinding = read(files.orderBinding);
const serviceHelpers = read(files.serviceHelpers);
const productPlan = read(files.productPlan);
const contentPlan = read(files.contentPlan);
const orderPlan = read(files.orderPlan);
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

for (const [label, plan] of [['product plan', productPlan], ['content plan', contentPlan], ['order plan', orderPlan]]) {
  assertIncludes(plan, 'Execution checkpoint', label);
  assertIncludes(plan, 'rustok-ai', label);
}
assertIncludes(productPlan, 'compile-free static verification', 'product plan evidence');
assertIncludes(contentPlan, 'compile-free static verification', 'content plan evidence');
assertIncludes(orderPlan, 'compile-free static verification', 'order plan evidence');
assertIncludes(aiPlan, 'scripts/verify/verify-ai-domain-verticals.mjs', 'rustok-ai plan evidence');

if (failures.length > 0) {
  console.error('AI domain vertical static verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log('AI domain vertical static verification passed');
