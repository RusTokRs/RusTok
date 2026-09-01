#!/usr/bin/env node

import fs from 'node:fs';

const failures = [];
const requireText = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}`);
};
const rejectText = (source, marker, label = marker) => {
  if (source.includes(marker)) failures.push(`must not contain ${label}`);
};
const normalizeWhitespace = (source) => source.replace(/\s+/g, ' ').trim();

const planPath = 'crates/rustok-forum/docs/implementation-plan.md';
const centralPlanPath = 'docs/modules/translation-implementation-plan.md';
const modulePlanPath = 'crates/rustok-translation/docs/implementation-plan.md';
const registryPath = 'docs/modules/translation-surfaces.json';
const parityPath = 'crates/rustok-forum/docs/cat5-category-taxonomy-browser-parity.md';
const retirementTest = 'crates/rustok-forum/tests/category_taxonomy_translation_provider_retirement.rs';
const retiredPaths = [
  'crates/rustok-forum/src/services/category_translation_target.rs',
  'crates/rustok-forum/src/services/category_translation_progress.rs',
  'crates/rustok-forum/tests/category_translation_target_postgres.rs',
];

for (const path of [planPath, centralPlanPath, modulePlanPath, registryPath, parityPath, retirementTest]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}
for (const path of retiredPaths) {
  if (fs.existsSync(path)) failures.push(`${path}: retired provider-era source must be absent`);
}

if (failures.length === 0) {
  const plan = fs.readFileSync(planPath, 'utf8');
  const centralPlan = fs.readFileSync(centralPlanPath, 'utf8');
  const modulePlan = fs.readFileSync(modulePlanPath, 'utf8');
  const normalizedCentralPlan = normalizeWhitespace(centralPlan);
  const normalizedModulePlan = normalizeWhitespace(modulePlan);
  const parity = fs.readFileSync(parityPath, 'utf8');
  const registry = JSON.parse(fs.readFileSync(registryPath, 'utf8'));

  requireText(
    plan,
    'Canonical Forum Category identity, localized copy, route\nhistory, hierarchy/order and presentation are Taxonomy-owned after the verified\nCAT-5 cutover',
    'Taxonomy-owned Forum Category boundary',
  );
  requireText(plan, "Category copy uses Taxonomy's registered `taxonomy/term` provider.");
  requireText(plan, 'never register a duplicate `forum/category` Translation target.');
  requireText(plan, 'provider-era PostgreSQL evidence is superseded');
  requireText(plan, 'Mounted multilingual/RTL browser execution is deferred to the final production-validation phase');
  requireText(plan, '| `FORUM-25` | `done` |', 'FORUM-25 implementation is complete');
  requireText(plan, '## Final production validation — DEFERRED', 'Forum final production-validation phase');
  requireText(plan, 'without reopening FORUM-25 or TAXONOMY-CAT-5 implementation', 'Forum Category deferred production evidence boundary');
  rejectText(plan, '- category hierarchy and policy;', 'stale Forum-owned Category hierarchy');
  rejectText(
    plan,
    'Forum category Translation provider, cursor/progress/PostgreSQL evidence',
    'stale Forum Category provider readiness claim',
  );
  rejectText(
    plan,
    'Retain registered-host/runtime provider evidence plus mounted multilingual/RTL browser parity',
    'retired Forum provider runtime gate',
  );

  requireText(
    normalizedCentralPlan,
    'Forum Category copy also follows the canonical Taxonomy provider through the same-ID Forum-to-Taxonomy Category binding',
    'central Translation plan Forum-to-Taxonomy provider boundary',
  );
  requireText(
    normalizedCentralPlan,
    'Blog Category/Taxonomy and Forum Category/Taxonomy ownership are resolved',
    'central Translation owner-drift status',
  );
  requireText(
    normalizedCentralPlan,
    'Forum Category canonical copy is not a Forum Translation target.',
    'central Forum onboarding exclusion',
  );
  requireText(
    normalizedCentralPlan,
    'including canonical Category copy consumed by Blog and Forum',
    'Taxonomy Category consumer scope',
  );
  requireText(
    normalizedCentralPlan,
    'Blog and Forum Category do not add a second provider or evidence gate',
    'central no-duplicate Category provider rule',
  );
  rejectText(normalizedCentralPlan, 'Category may onboard early;', 'stale direct Forum Category onboarding');
  rejectText(
    normalizedCentralPlan,
    'do not reintroduce a Blog-local Category Translation owner',
    'Blog-only Category ownership guard wording',
  );

  requireText(
    normalizedModulePlan,
    'Blog Category canonical copy is consumed through the same-ID Blog-to-Taxonomy Category binding and the `taxonomy/term` provider',
    'module Translation plan Blog-to-Taxonomy provider boundary',
  );
  requireText(
    normalizedModulePlan,
    'Forum Category canonical copy is consumed through the same-ID Forum-to-Taxonomy Category binding and the same `taxonomy/term` provider',
    'module Translation plan Forum-to-Taxonomy provider boundary',
  );
  requireText(
    normalizedModulePlan,
    'The duplicate `forum/category` provider, Forum Category change/progress runtime, and Forum-local donor translation storage are retired and must not be recreated',
    'module Translation plan retired Forum provider boundary',
  );
  requireText(
    normalizedModulePlan,
    'Forum topic/reply Translation remains a separate opt-in UGC onboarding track',
    'module Translation plan Forum UGC separation',
  );
  rejectText(
    normalizedModulePlan,
    'Category binding and the `taxonomy/term` provider; the former `blog/category` provider',
    'Blog-only module Category provider boundary',
  );

  requireText(parity, 'The backend ownership/storage cutover is already complete');
  requireText(parity, 'Status: **executable browser source / maintainer execution pending**');

  const forum = registry.surfaces?.find((surface) => surface.id === 'forum_categories');
  const taxonomy = registry.surfaces?.find((surface) => surface.id === 'taxonomy_terms');
  if (!forum) {
    failures.push('translation registry: forum_categories surface is required');
  } else {
    if (forum.readiness !== 'excluded') failures.push('forum_categories.readiness must be excluded');
    if (forum.provider_status !== 'not_registered') {
      failures.push('forum_categories.provider_status must be not_registered');
    }
    if (forum.ai_export !== 'forbidden') failures.push('forum_categories.ai_export must be forbidden');
    if (!forum.field_profiles?.includes('slug')) failures.push('forum_categories must retain slug field classification');
    for (const path of retiredPaths) {
      if (forum.evidence_paths?.includes(path)) failures.push(`forum_categories must not cite retired path ${path}`);
    }
    for (const path of [
      retirementTest,
      'docs/architecture/taxonomy-flex-category-platform-plan.md',
      'crates/rustok-taxonomy/src/translation_target.rs',
    ]) {
      if (!forum.evidence_paths?.includes(path)) failures.push(`forum_categories missing evidence ${path}`);
    }
    if (!forum.exclusion_reason?.includes('Taxonomy-owned')) {
      failures.push('forum_categories.exclusion_reason must state Taxonomy ownership');
    }
    if (!forum.exclusion_reason?.includes('must not register a duplicate forum/category Translation target')) {
      failures.push('forum_categories.exclusion_reason must forbid duplicate forum/category registration');
    }
  }
  if (!taxonomy || taxonomy.provider_status !== 'registered') {
    failures.push('taxonomy_terms must remain the registered canonical provider');
  }
}

if (failures.length > 0) {
  console.error('[forum-taxonomy-category-translation-owner] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[forum-taxonomy-category-translation-owner] Forum Category ownership matches Taxonomy cutover');
