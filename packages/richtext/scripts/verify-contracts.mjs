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
for (const required of ['RichTextDocument', "'article' | 'discussion' | 'comment'"]) {
  if (!contracts.includes(required)) throw new Error(`Generated contracts are missing ${required}`);
}
console.log('Richtext browser contracts match the Rust fixture and structural schema.');
