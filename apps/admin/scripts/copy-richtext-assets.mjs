import { cp, mkdir, readFile, rm } from 'node:fs/promises';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const adminRoot = resolve(import.meta.dirname, '..');
const repositoryRoot = resolve(adminRoot, '../..');
const packageRoot = resolve(repositoryRoot, 'packages/richtext');
const packageDist = resolve(packageRoot, 'dist');
const targetRoot = resolve(adminRoot, 'dist/richtext/frame');

if (!(await exists(resolve(packageDist, 'asset-manifest.json')))) {
  const result = spawnSync('npm.cmd', ['run', 'build', '--prefix', packageRoot], {
    cwd: repositoryRoot,
    stdio: 'inherit',
    shell: true
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

const manifest = JSON.parse(await readFile(resolve(packageDist, 'asset-manifest.json'), 'utf8'));
await rm(targetRoot, { recursive: true, force: true });
await mkdir(targetRoot, { recursive: true });
for (const asset of [manifest.script, manifest.style]) {
  await cp(resolve(packageDist, asset), resolve(targetRoot, asset));
}
await cp(resolve(packageDist, manifest.frame), resolve(targetRoot, 'index.html'));
await cp(resolve(packageDist, 'asset-manifest.json'), resolve(targetRoot, 'asset-manifest.json'));
await cp(resolve(packageDist, 'leptos-adapter.mjs'), resolve(targetRoot, 'leptos-adapter.mjs'));

async function exists(path) {
  try {
    await readFile(path);
    return true;
  } catch {
    return false;
  }
}
