#!/usr/bin/env node
import { existsSync, readFileSync } from 'node:fs';

const failures = [];
const read = (path) => readFileSync(path, 'utf8');
const fail = (message) => failures.push(message);
const requireFile = (path) => {
  if (!existsSync(path)) fail(`${path}: missing required file`);
};
const requireText = (text, marker, label) => {
  if (!text.includes(marker)) fail(`${label}: missing ${marker}`);
};
const forbidText = (text, marker, label) => {
  if (text.includes(marker)) fail(`${label}: forbidden ${marker}`);
};

const files = {
  aiCargo: 'crates/rustok-ai/Cargo.toml',
  aiPort: 'crates/rustok-ai/src/ports.rs',
  aiLocalePolicy: 'crates/rustok-ai/src/service/helpers.rs',
  translationCargo: 'crates/rustok-translation/Cargo.toml',
  translationPort: 'crates/rustok-translation/src/machine.rs',
  adapterCargo: 'crates/rustok-ai-translation/Cargo.toml',
  adapterSource: 'crates/rustok-ai-translation/src/lib.rs',
  adapterReadme: 'crates/rustok-ai-translation/README.md',
  adapterPlan: 'crates/rustok-ai-translation/docs/implementation-plan.md',
};

for (const file of Object.values(files)) requireFile(file);

const aiCargo = read(files.aiCargo);
const aiPort = read(files.aiPort);
const aiLocalePolicy = read(files.aiLocalePolicy);
const translationCargo = read(files.translationCargo);
const translationPort = read(files.translationPort);
const adapterCargo = read(files.adapterCargo);
const adapterSource = read(files.adapterSource);
const adapterReadme = read(files.adapterReadme);
const adapterPlan = read(files.adapterPlan);
const hasDependency = (manifest, dependency) =>
  new RegExp(`^\\s*${dependency}(?:\\.workspace\\s*=|\\s*=)`, 'm').test(manifest);

if (hasDependency(aiCargo, 'rustok-translation')) {
  fail('rustok-ai must not depend on rustok-translation');
}
if (hasDependency(translationCargo, 'rustok-ai')) {
  fail('rustok-translation must not depend on rustok-ai');
}
if (!hasDependency(adapterCargo, 'rustok-ai')) {
  fail('rustok-ai-translation must depend on rustok-ai');
}
if (!hasDependency(adapterCargo, 'rustok-translation')) {
  fail('rustok-ai-translation must depend on rustok-translation');
}

for (const marker of [
  'pub trait AiStructuredTaskPort',
  'PortCallPolicy::write()',
  'AiStructuredTaskExecution',
  'AiStructuredTaskUsage',
  'async fn status(',
  'async fn cancel(',
]) requireText(aiPort, marker, 'AI structured-task port');

for (const marker of [
  'pub trait MachineTranslationPort',
  'MachineTranslationBatchRequest',
  'MachineTranslationExecutionEvidence',
  'review_required',
]) requireText(translationPort, marker, 'Translation machine port');

for (const marker of [
  'MACHINE_TRANSLATION_TASK_SLUG: &str = "machine_translation"',
  'impl MachineTranslationPort for AiMachineTranslationAdapter',
  'AiStructuredTaskRequest',
  'output_unit_missing',
  'output_tokens_changed',
  'review_required: true',
]) requireText(adapterSource, marker, 'AI Translation adapter');

for (const marker of [
  'sea_orm',
  'AiManagementService',
  'InferenceEngine',
  'apps::server',
  'rustok_product',
  'rustok_media',
  'rustok_blog',
  'rustok_commerce',
  'graphql',
]) forbidText(adapterSource, marker, 'stateless adapter boundary');

requireText(
  aiLocalePolicy,
  'assert!(!task_allows_free_locale("translation"));',
  'legacy translation locale alias removal',
);
forbidText(
  aiLocalePolicy,
  '| "translation"',
  'legacy translation free-locale allow-list',
);
requireText(adapterReadme, 'never mutates', 'adapter ownership docs');
requireText(adapterPlan, 'Live runtime registration is intentionally absent', 'adapter gate docs');

if (failures.length > 0) {
  console.error('AI Translation boundary verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('AI Translation ownership, dependency, and activation boundary verification passed');
