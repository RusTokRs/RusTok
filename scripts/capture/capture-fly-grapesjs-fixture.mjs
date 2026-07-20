#!/usr/bin/env node

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const nextAdminRoot = path.join(repositoryRoot, 'apps/next-admin');
const fixtureRoot = path.join(repositoryRoot, 'crates/fly/fixtures/grapesjs');
const manifestPath = path.join(fixtureRoot, 'manifest.json');
const seedFile = process.env.FLY_GRAPESJS_CAPTURE_SEED || 'baseline.json';
const outputFile = process.env.FLY_GRAPESJS_CAPTURE_OUTPUT || 'browser-current.json';
const appRequire = createRequire(path.join(nextAdminRoot, 'package.json'));
const { chromium } = appRequire('@playwright/test');
const grapesJsPath = appRequire.resolve('grapesjs/dist/grapes.min.js');
const presetPath = appRequire.resolve('grapesjs-preset-webpage');
const grapesJsVersion = appRequire('grapesjs/package.json').version;
const presetVersion = appRequire('grapesjs-preset-webpage/package.json').version;
const seed = JSON.parse(await readFile(path.join(fixtureRoot, seedFile), 'utf8'));
const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
const captureCommit =
  process.env.FLY_GRAPESJS_CAPTURE_COMMIT ||
  execFileSync('git', ['rev-parse', 'HEAD'], {
    cwd: repositoryRoot,
    encoding: 'utf8'
  }).trim();
const capturedAt = process.env.FLY_GRAPESJS_CAPTURED_AT || new Date().toISOString();
const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || undefined;
const browser = await chromium.launch({ headless: true, executablePath });

try {
  const browserVersion = browser.version();
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
  }, seed);

  assert.deepEqual(result.second, result.first, 'captured GrapesJS project is not idempotent');
  await writeFile(
    path.join(fixtureRoot, outputFile),
    `${JSON.stringify(result.first, null, 2)}\n`,
    'utf8'
  );

  const metadata = {
    file: outputFile,
    seed: seedFile,
    source: 'playwright-chromium-getProjectData',
    real_browser_capture: true,
    current_runtime: true,
    purpose: 'Real GrapesJS browser normalization and bidirectional reload baseline',
    grapesjs_version: grapesJsVersion,
    grapesjs_preset_webpage_version: presetVersion,
    browser: `chromium ${browserVersion}`,
    capture_commit: captureCommit,
    plugins: [`grapesjs-preset-webpage@${presetVersion}`],
    captured_at: capturedAt
  };
  manifest.fixtures = manifest.fixtures.filter((fixture) => fixture.file !== outputFile);
  manifest.fixtures.push(metadata);
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
  await page.close();

  console.log(
    `captured ${outputFile} with GrapesJS ${grapesJsVersion}, preset ${presetVersion}, Chromium ${browserVersion}`
  );
} finally {
  await browser.close();
}
