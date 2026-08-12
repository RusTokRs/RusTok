import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(packageRoot, '../..');
const sourceProfiles = JSON.parse(
  await readFile(resolve(repositoryRoot, 'crates/rustok-content/fixtures/richtext/profiles.json'), 'utf8')
);
const generatedProfiles = JSON.parse(
  await readFile(resolve(packageRoot, 'src/generated/profiles.json'), 'utf8')
);
if (JSON.stringify(sourceProfiles) !== JSON.stringify(generatedProfiles)) {
  throw new Error('Generated richtext profiles drifted from the canonical fixture; run npm run generate:contracts');
}
const schema = JSON.parse(await readFile(resolve(packageRoot, 'src/generated/document.schema.json'), 'utf8'));
if (schema?.title !== 'RichTextDocument' || schema?.properties?.type?.const !== 'doc') {
  throw new Error('Generated RichTextDocument schema is missing the root contract');
}
const contracts = await readFile(resolve(packageRoot, 'src/generated/contracts.ts'), 'utf8');
for (const required of ['RichTextDocument', 'RichTextView', "'article' | 'discussion' | 'comment'"]) {
  if (!contracts.includes(required)) throw new Error(`Generated contracts are missing ${required}`);
}
const viewAdapter = await readFile(resolve(packageRoot, 'src/view.tsx'), 'utf8');
for (const required of ['view: RichTextView', 'contentLocale: string', 'dangerouslySetInnerHTML={{ __html: view.html }}']) {
  if (!viewAdapter.includes(required)) throw new Error(`Editor-free richtext view is missing ${required}`);
}
for (const forbidden of ['@tiptap', './editor', './frame/controller', 'html: string;']) {
  if (viewAdapter.includes(forbidden)) throw new Error(`Editor-free richtext view contains forbidden ${forbidden}`);
}
const packageJson = JSON.parse(await readFile(resolve(packageRoot, 'package.json'), 'utf8'));
if (packageJson.exports?.['./view'] !== './src/view.tsx') {
  throw new Error('Package exports are missing the editor-free ./view boundary');
}
console.log('Richtext browser contracts match the Rust fixture and structural schema.');
