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
const grapesJsVersion = appRequire('grapesjs/package.json').version;
const presetVersion = appRequire('grapesjs-preset-webpage/package.json').version;
const fixtureRoot = path.join(repositoryRoot, 'crates/fly/fixtures/grapesjs');
const manifest = JSON.parse(await readFile(path.join(fixtureRoot, 'manifest.json'), 'utf8'));
const allowedNormalizationIds = new Set(['drop_empty_frame_head']);

validateManifest(manifest);
const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || undefined;
const browser = await chromium.launch({ headless: true, executablePath });

try {
  for (const fixtureMetadata of manifest.fixtures) {
    validateFixtureMetadata(fixtureMetadata);
    const fixture = JSON.parse(
      await readFile(path.join(fixtureRoot, fixtureMetadata.file), 'utf8')
    );
    if (fixtureMetadata.seed) {
      JSON.parse(await readFile(path.join(fixtureRoot, fixtureMetadata.seed), 'utf8'));
    }
    if (fixtureMetadata.browser_roundtrip === false) {
      console.log(
        `skipped GrapesJS runtime round trip: ${fixtureMetadata.file} [Fly codec only]`
      );
      continue;
    }

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
    assertPreserved(
      applyAllowedNormalizations(fixture, fixtureMetadata),
      result.first,
      fixtureMetadata.file
    );
    await page.close();
    console.log(
      `verified GrapesJS round trip: ${fixtureMetadata.file}` +
        (fixtureMetadata.real_browser_capture ? ' [browser capture]' : ' [structural baseline]')
    );
  }
} finally {
  await browser.close();
}

function validateManifest(value) {
  assert.equal(value.format, 'grapesjs', 'fixture manifest format must be grapesjs');
  assert.ok(Array.isArray(value.fixtures), 'fixture manifest must contain fixtures');
  assert.ok(value.fixtures.length > 0, 'fixture manifest must not be empty');
  const names = value.fixtures.map((fixture) => fixture.file);
  assert.equal(new Set(names).size, names.length, 'fixture filenames must be unique');
  assert.ok(
    value.fixtures.some((fixture) => fixture.real_browser_capture === true),
    'fixture manifest must contain at least one real GrapesJS browser capture'
  );
  assert.deepEqual(
    value.required_real_capture_metadata,
    [
      'grapesjs_version',
      'grapesjs_preset_webpage_version',
      'browser',
      'capture_commit',
      'plugins',
      'captured_at'
    ],
    'required real capture metadata contract changed unexpectedly'
  );
}

function validateFixtureMetadata(metadata) {
  assert.equal(typeof metadata.file, 'string', 'fixture file must be a string');
  assert.ok(metadata.file.endsWith('.json'), `${metadata.file}: fixture must be JSON`);
  assert.equal(typeof metadata.source, 'string', `${metadata.file}: source is required`);
  assert.equal(typeof metadata.purpose, 'string', `${metadata.file}: purpose is required`);
  assert.equal(
    typeof metadata.browser_roundtrip,
    'boolean',
    `${metadata.file}: browser_roundtrip must be explicit`
  );
  const normalizations = metadata.allowed_browser_normalizations ?? [];
  assert.ok(
    Array.isArray(normalizations),
    `${metadata.file}: allowed_browser_normalizations must be an array`
  );
  for (const normalization of normalizations) {
    assert.ok(
      allowedNormalizationIds.has(normalization),
      `${metadata.file}: unknown allowed browser normalization ${normalization}`
    );
  }
  if (!metadata.browser_roundtrip) {
    assert.equal(
      normalizations.length,
      0,
      `${metadata.file}: skipped browser fixtures cannot declare browser normalizations`
    );
    assert.equal(
      typeof metadata.browser_skip_reason,
      'string',
      `${metadata.file}: browser_skip_reason is required`
    );
    assert.ok(
      metadata.browser_skip_reason.trim().length > 0,
      `${metadata.file}: browser_skip_reason must not be empty`
    );
  }
  if (!metadata.real_browser_capture) {
    return;
  }

  assert.equal(
    metadata.browser_roundtrip,
    true,
    `${metadata.file}: real browser captures must run through the browser verifier`
  );
  assert.equal(
    normalizations.length,
    0,
    `${metadata.file}: real browser captures cannot declare structural normalizations`
  );
  for (const field of manifest.required_real_capture_metadata) {
    assert.ok(metadata[field], `${metadata.file}: missing real capture metadata ${field}`);
  }
  assert.match(
    metadata.capture_commit,
    /^[0-9a-f]{40}$/,
    `${metadata.file}: capture_commit must be a full Git SHA`
  );
  assert.ok(
    Number.isFinite(Date.parse(metadata.captured_at)),
    `${metadata.file}: captured_at must be an ISO-compatible timestamp`
  );
  assert.ok(Array.isArray(metadata.plugins), `${metadata.file}: plugins must be an array`);
  assert.ok(metadata.plugins.length > 0, `${metadata.file}: plugins must not be empty`);
  assert.equal(typeof metadata.browser, 'string', `${metadata.file}: browser must be a string`);
  assert.ok(metadata.browser.trim().length > 0, `${metadata.file}: browser must not be empty`);
  if (metadata.current_runtime) {
    assert.equal(
      metadata.grapesjs_version,
      grapesJsVersion,
      `${metadata.file}: current GrapesJS capture version is stale`
    );
    assert.equal(
      metadata.grapesjs_preset_webpage_version,
      presetVersion,
      `${metadata.file}: current preset capture version is stale`
    );
    assert.ok(
      metadata.plugins.includes(`grapesjs-preset-webpage@${presetVersion}`),
      `${metadata.file}: current preset plugin metadata is stale`
    );
  }
}

function applyAllowedNormalizations(fixture, metadata) {
  const expected = structuredClone(fixture);
  for (const normalization of metadata.allowed_browser_normalizations ?? []) {
    if (normalization === 'drop_empty_frame_head') {
      for (const page of expected.pages ?? []) {
        for (const frame of page.frames ?? []) {
          if (Array.isArray(frame.head) && frame.head.length === 0) {
            delete frame.head;
          }
        }
      }
    }
  }
  return expected;
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
