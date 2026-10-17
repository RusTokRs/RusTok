import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const json = (path) =>
  JSON.parse(readFileSync(new URL(path, import.meta.url), 'utf8'));
const manifest = json('../package.json');
const lock = json('../package-lock.json');

test('richtext declares and locks one compatible Tiptap release cohort', () => {
  const version = manifest.dependencies['@tiptap/core'];
  assert.match(version, /^\d+\.\d+\.\d+$/);
  for (const [name, value] of Object.entries(manifest.dependencies)) {
    if (name.startsWith('@tiptap/')) assert.equal(value, version, name);
  }
  assert.deepEqual(lock.packages[''].dependencies, manifest.dependencies);
  for (const [path, pkg] of Object.entries(lock.packages)) {
    if (path.includes('node_modules/@tiptap/'))
      assert.equal(pkg.version, version, path);
  }
});

for (const host of ['next-frontend', 'next-admin']) {
  test(`${host} locks the current richtext dependency contract and matching React renderer/types`, () => {
    const hostManifest = json(`../../../apps/${host}/package.json`);
    const hostLock = json(`../../../apps/${host}/package-lock.json`);
    assert.deepEqual(
      hostLock.packages['../../packages/richtext'].dependencies,
      manifest.dependencies,
    );
    const dependencies = {
      ...hostManifest.dependencies,
      ...hostManifest.devDependencies,
    };
    assert.equal(dependencies.react, dependencies['react-dom']);
    assert.equal(
      dependencies['@types/react'],
      dependencies['@types/react-dom'],
    );
    for (const name of [
      'react',
      'react-dom',
      '@types/react',
      '@types/react-dom',
      'typescript',
      'eslint',
      'next',
      'eslint-config-next',
    ]) {
      assert.equal(
        hostLock.packages[`node_modules/${name}`].version,
        dependencies[name],
        name,
      );
    }
  });
}

test('Next hosts retain build-time type checks and admin installs use the canonical npm lock', () => {
  for (const host of ['next-frontend', 'next-admin']) {
    const config = readFileSync(
      new URL(`../../../apps/${host}/next.config.mjs`, import.meta.url),
      'utf8',
    );
    assert.doesNotMatch(config, /ignoreBuildErrors\s*:\s*true/);
  }
  const docker = readFileSync(
    new URL('../../../apps/next-admin/Dockerfile', import.meta.url),
    'utf8',
  );
  assert.match(docker, /RUN npm ci/);
  assert.match(docker, /COPY packages\/richtext\//);
  assert.doesNotMatch(docker, /bun|--force|--legacy-peer-deps/);
});
