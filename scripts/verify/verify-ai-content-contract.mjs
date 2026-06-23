#!/usr/bin/env node
import { readFileSync } from 'node:fs';

const source = readFileSync('crates/rustok-ai-content/src/lib.rs', 'utf8');
const plan = readFileSync('crates/rustok-ai-content/docs/implementation-plan.md', 'utf8');
const readme = readFileSync('crates/rustok-ai-content/docs/README.md', 'utf8');

function assertIncludes(haystack, needle, label) {
  if (!haystack.includes(needle)) {
    throw new Error(`${label} is missing required marker: ${needle}`);
  }
}

for (const field of ['title', 'slug', 'body', 'excerpt', 'seo_title', 'seo_description']) {
  assertIncludes(source, `("${field}", payload.${field}.as_deref())`, `GeneratedBlogDraft validation for ${field}`);
}

for (const testName of [
  'accepts_full_blog_draft_payload_contract',
  'accepts_empty_blog_draft_payload_for_patch_style_generation',
  'rejects_blank_blog_draft_fields_when_provided',
]) {
  assertIncludes(source, `fn ${testName}`, `blog draft contract test ${testName}`);
}

assertIncludes(source, 'CONTENT_AI_POLICY_MATRIX', 'content AI policy matrix');
assertIncludes(source, 'ContentAiApprovalMode::OperatorApproval', 'moderation approval routing');
assertIncludes(source, 'ContentAiApprovalMode::Auto', 'blog draft auto routing');
assertIncludes(plan, 'blog_contract_static_evidence_added', 'implementation plan checkpoint');
assertIncludes(readme, 'all optional text fields', 'docs validation summary');

console.log('AI content contract static verification passed');
