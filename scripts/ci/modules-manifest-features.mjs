#!/usr/bin/env node
// Derives the `rustok-server` cargo feature set that compiles exactly the modules a `modules.toml`
// manifest declares, and checks the default feature list against it.
//
// `modules.toml` is the canonical source of module composition. The server validates the manifest
// against the *compiled* registry at startup (`validate_registry_vs_manifest`): a module whose
// package declares a `[crate] entry_type` must be in the registry, and a registry entry the
// manifest does not declare is rejected. So the default feature list has to cover exactly the
// modules the repository's own manifest declares. When it did not, `cargo build -p rustok-server`
// produced a binary that exited during startup with `MissingInRegistry`, which is what made every
// step of the "Rust-hosted Browser Smoke" job after its build unreachable.
//
// Usage:
//   node scripts/ci/modules-manifest-features.mjs --print      # feature list, for CI
//   node scripts/ci/modules-manifest-features.mjs --check      # compare with the Cargo.toml default
//   node scripts/ci/modules-manifest-features.mjs --print --manifest <path>
//
// Exit code: 0 when the check passes (or for --print), 1 when the default list and the manifest
// disagree, 2 on a usage or read error.

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (relativePath) => readFileSync(path.join(repoRoot, relativePath), 'utf8');

/// The `[modules]` table of a manifest, as `slug -> inline table body`.
///
/// The table is one `key = { ... }` per line; anything else is a comment or a section header.
/// Parsing it here rather than with a TOML library keeps this script dependency-free, which is
/// what lets CI run it before any install step.
export function manifestModules(source) {
  const lines = source.split(/\r?\n/);
  const start = lines.findIndex((line) => line.trim() === '[modules]');
  if (start < 0) throw new Error('manifest has no [modules] table');
  const modules = new Map();
  for (const line of lines.slice(start + 1)) {
    const trimmed = line.trim();
    if (trimmed.startsWith('[')) break;
    if (!trimmed || trimmed.startsWith('#')) continue;
    const match = /^([A-Za-z0-9_-]+)\s*=\s*\{(.*)\}\s*$/.exec(trimmed);
    if (!match) continue;
    modules.set(match[1], match[2]);
  }
  return modules;
}

/// Fields of an inline table body, as `key -> raw value`.
function inlineFields(body) {
  const fields = new Map();
  for (const part of body.split(',')) {
    const match = /^\s*([A-Za-z0-9_-]+)\s*=\s*(.+?)\s*$/.exec(part);
    if (match) fields.set(match[1], match[2]);
  }
  return fields;
}

const unquote = (value) => value.replace(/"/g, '').trim();
const featureName = (slug) => `mod-${slug}`;

/// Whether the module package declares a runtime entry (`[crate] entry_type`).
///
/// This is what `apply_module_package_manifest` lifts into the spec the registry contract is
/// checked against, so it — not the manifest line alone — decides whether the module is required
/// in the compiled registry. A module without an entry (a capability extension) is never required.
export function packageHasEntryType(slug) {
  for (const directory of [`crates/modules/rustok-${slug}`, `crates/modules/${slug}`]) {
    let source;
    try {
      source = readFileSync(path.join(repoRoot, directory, 'rustok-module.toml'), 'utf8');
    } catch {
      continue;
    }
    const lines = source.split(/\r?\n/);
    const crateSection = lines.findIndex((line) => line.trim() === '[crate]');
    if (crateSection < 0) return false;
    for (const line of lines.slice(crateSection + 1)) {
      if (line.trim().startsWith('[')) break;
      if (/^\s*entry_type\s*=/.test(line)) return true;
    }
    return false;
  }
  return false;
}

/// Every slug the manifest declares, whatever its runtime mode.
///
/// The registry contract is about the manifest map, so a `mod-*` feature that compiles a declared
/// module — an extension included — is consistent rather than unexpected.
export function manifestSlugs(source) {
  return new Set(manifestModules(source).keys());
}

/// Whether the manifest entry is compiled from this workspace at all.
///
/// A `runtime = "extension"` entry is hosted by the runtime-extension seam and a non-`path` source
/// is not built here; neither contributes a `mod-*` feature requirement.
function compilesIntoRegistry(fields) {
  if (unquote(fields.get('runtime') ?? '') === 'extension') return false;
  return unquote(fields.get('source') ?? '"path"') === 'path';
}

/// Features that must be enabled to compile the manifest's modules.
///
/// Requesting a `mod-*` feature the server does not declare is a hard cargo error, so this reports
/// only declared features and records a compiled manifest module that has none.
export function requiredFeatures(manifestSource, serverFeatureNames) {
  const required = new Set();
  const missingFeature = [];
  for (const [slug, body] of manifestModules(manifestSource)) {
    const fields = inlineFields(body);
    if (!compilesIntoRegistry(fields)) continue;
    if (!packageHasEntryType(slug)) continue;
    // A `required = true` module is registered unconditionally by `rustok-distribution`.
    if (unquote(fields.get('required') ?? '') === 'true') continue;
    const feature = featureName(slug);
    if (serverFeatureNames.has(feature)) required.add(feature);
    else missingFeature.push(slug);
  }
  return { required, missingFeature };
}

/// The `default = [ ... ]` list of a Cargo.toml, plus every feature the file declares.
export function serverFeatures(source) {
  const declared = new Set();
  for (const match of source.matchAll(/^([A-Za-z0-9_-]+)\s*=\s*\[/gm)) declared.add(match[1]);
  const defaultMatch = /^default\s*=\s*\[([\s\S]*?)\]/m.exec(source);
  if (!defaultMatch) throw new Error('apps/server/Cargo.toml has no default feature list');
  const defaults = [...defaultMatch[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]);
  return { declared, defaults };
}

export function compare(manifestSource, serverSource) {
  const { declared, defaults } = serverFeatures(serverSource);
  const { required, missingFeature } = requiredFeatures(manifestSource, declared);
  const slugs = manifestSlugs(manifestSource);
  const defaultSet = new Set(defaults);
  return {
    required,
    missingFeature,
    // In the manifest, entry-bearing, and not in the default build: the binary cannot start.
    missingFromDefault: [...required].filter((feature) => !defaultSet.has(feature)).sort(),
    // In the default build and never declared: startup validation rejects it.
    absentFromManifest: [...defaultSet]
      .filter((feature) => feature.startsWith('mod-'))
      .map((feature) => feature.slice('mod-'.length))
      .filter((slug) => !slugs.has(slug))
      .sort(),
  };
}

function parseArgs(argv) {
  const usage = (message) => {
    if (message) console.error(message);
    console.error(
      'usage: node scripts/ci/modules-manifest-features.mjs [--print|--check] [--manifest <path>]',
    );
    process.exit(2);
  };
  let mode = null;
  let manifest = path.join(repoRoot, 'modules.toml');
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--print' || argument === '--check') {
      if (mode) usage('choose one of --print and --check');
      mode = argument.slice(2);
    } else if (argument === '--manifest') {
      const value = argv[(index += 1)];
      if (!value) usage('--manifest requires a path');
      manifest = path.isAbsolute(value) ? value : path.join(repoRoot, value);
    } else if (argument === '--help' || argument === '-h') {
      usage();
    } else {
      usage(`unknown argument: ${argument}`);
    }
  }
  if (!mode) usage('choose one of --print and --check');
  return { mode, manifest };
}

function main() {
  const { mode, manifest: manifestPath } = parseArgs(process.argv.slice(2));
  let manifestSource;
  try {
    manifestSource = readFileSync(manifestPath, 'utf8');
  } catch (error) {
    console.error(`cannot read ${manifestPath}: ${error.message}`);
    process.exit(2);
  }
  const result = compare(manifestSource, read('apps/server/Cargo.toml'));

  if (mode === 'print') {
    // A stable order keeps CI logs and cache keys identical between runs.
    console.log(
      [...result.required].sort().join(',') +
        (result.missingFeature.length
          ? `\n# manifest modules without a declared feature: ${result.missingFeature.join(', ')}`
          : ''),
    );
    process.exit(0);
  }

  const failures = [];
  for (const slug of result.missingFeature) {
    failures.push(
      `manifest module \`${slug}\` declares a runtime entry but apps/server has no ` +
        `\`${featureName(slug)}\` feature; the binary cannot include it`,
    );
  }
  for (const feature of result.missingFromDefault) {
    failures.push(
      `\`${feature}\` is required by modules.toml and is not in the apps/server default feature ` +
        'list; `cargo build -p rustok-server` produces a binary that fails ' +
        '`validate_registry_vs_manifest` at startup',
    );
  }
  for (const slug of result.absentFromManifest) {
    failures.push(
      `\`${featureName(slug)}\` is in the apps/server default feature list and modules.toml does ` +
        'not declare the module; startup validation rejects it as `UnexpectedInRegistry`',
    );
  }
  if (failures.length) {
    console.error('[modules-manifest-features] FAIL');
    failures.forEach((failure) => console.error(`- ${failure}`));
    process.exit(1);
  }
  console.log(
    `[modules-manifest-features] PASS manifest_modules=${result.required.size} ` +
      'default_features_match=true',
  );
}

// Run only as a script: the tests import this module, and a test runner's `argv` is not its CLI.
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
