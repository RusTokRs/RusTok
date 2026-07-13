#!/usr/bin/env node

import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const nextAdminRoot = path.join(repositoryRoot, 'apps/next-admin');
const appRequire = createRequire(path.join(nextAdminRoot, 'package.json'));
const { chromium } = appRequire('@playwright/test');
const grapesJsPath = appRequire.resolve('grapesjs/dist/grapes.min.js');
const presetPath = appRequire.resolve('grapesjs-preset-webpage');
const fixtureRoot = path.join(repositoryRoot, 'crates/fly/fixtures/grapesjs');
const manifest = JSON.parse(await readFile(path.join(fixtureRoot, 'manifest.json'), 'utf8'));

const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || undefined;
const browser = await chromium.launch({ headless: true, executablePath });

try {
  for (const fixtureMetadata of manifest.fixtures) {
    const fixture = JSON.parse(
      await readFile(path.join(fixtureRoot, fixtureMetadata.file), 'utf8')
    );
    const page = await browser.newPage();
    await page.setContent('<div id="editor"></div>');
    await page.addScriptTag({ path: grapesJsPath });
    await page.addScriptTag({ path: presetPath });

    const result = await page.evaluate((projectData) => {
      const grapesjs = globalThis.grapesjs;
      const preset =
        globalThis.grapesjsPresetWebpage ??
        globalThis['grapesjs-preset-webpage'];
      if (!grapesjs) {
        throw new Error('GrapesJS browser global is unavailable');
      }

      const editor = grapesjs.init({
        container: '#editor',
        height: '100px',
        width: '100px',
        storageManager: false,
        noticeOnUnload: false,
        fromElement: false,
        plugins: preset ? [preset] : []
      });

      editor.loadProjectData(projectData);
      const first = editor.getProjectData();
      editor.loadProjectData(first);
      const second = editor.getProjectData();
      editor.destroy();
      return { first, second };
    }, fixture);

    assert.deepEqual(
      result.second,
      result.first,
      `${fixtureMetadata.file}: GrapesJS load/get cycle is not idempotent`
    );
    assertPreserved(fixture, result.first, fixtureMetadata.file);
    await page.close();
    console.log(`verified GrapesJS round trip: ${fixtureMetadata.file}`);
  }
} finally {
  await browser.close();
}

function assertPreserved(expected, actual, location) {
  if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual), `${location}: expected array`);
    assert.equal(actual.length, expected.length, `${location}: array length changed`);
    expected.forEach((value, index) =>
      assertPreserved(value, actual[index], `${location}[${index}]`)
    );
    return;
  }

  if (expected && typeof expected === 'object') {
    assert.ok(actual && typeof actual === 'object', `${location}: expected object`);
    for (const [key, value] of Object.entries(expected)) {
      assert.ok(
        Object.prototype.hasOwnProperty.call(actual, key),
        `${location}.${key}: field was dropped`
      );
      assertPreserved(value, actual[key], `${location}.${key}`);
    }
    return;
  }

  assert.deepEqual(actual, expected, `${location}: value changed`);
}
