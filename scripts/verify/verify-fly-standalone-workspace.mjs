#!/usr/bin/env node

// `crates/ui/fly/standalone-Cargo.toml` is the workspace template used when Fly is extracted into
// its own repository. Every Fly sub-crate inherits `version`, `edition`, `license` and most of its
// dependencies from the workspace, so the template is only useful if it actually declares
// `[workspace.package]` and `[workspace.dependencies]` — and if those pins do not drift away from
// the host workspace.
//
// This is deliberately a textual check: the thing being verified *is* a manifest's text, there is
// no behaviour to exercise, and the alternative (extracting and building the tree) belongs in a
// slower job.

import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import process from 'node:process';

const root = resolve(import.meta.dirname, '../..');

const read = async (path) => readFile(resolve(root, path), 'utf8');

// Comments routinely mention section names (this template documents `[workspace.dependencies]` in
// its header), so strip them before locating anything.
const withoutComments = (manifest) =>
  manifest
    .split('\n')
    .map((line) => (line.trimStart().startsWith('#') ? '' : line))
    .join('\n');

const section = (manifest, name) => {
  const stripped = withoutComments(manifest);
  const header = `[${name}]`;
  const start = stripped.indexOf(header);
  if (start === -1) return null;
  const rest = stripped.slice(start + header.length);
  const end = rest.search(/^\[/m);
  return end === -1 ? rest : rest.slice(0, end);
};

const hasSection = (manifest, name) => section(manifest, name) !== null;

const entries = (body) => {
  const found = new Map();
  if (!body) return found;
  for (const line of body.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    const separator = trimmed.indexOf('=');
    if (separator === -1) continue;
    found.set(trimmed.slice(0, separator).trim(), trimmed.slice(separator + 1).trim());
  }
  return found;
};

const failures = [];

const workspace = await read('Cargo.toml');
const standalone = await read('crates/ui/fly/standalone-Cargo.toml');

// 1. The template must declare the inherited sections at all.
for (const name of ['workspace', 'workspace.package', 'workspace.dependencies']) {
  if (!hasSection(standalone, name)) {
    failures.push(`standalone-Cargo.toml is missing the [${name}] section`);
  }
}

const standalonePackage = entries(section(standalone, 'workspace.package'));
const workspacePackage = entries(section(workspace, 'workspace.package'));
const standaloneDeps = entries(section(standalone, 'workspace.dependencies'));
const workspaceDeps = entries(section(workspace, 'workspace.dependencies'));

// 2. Everything the Fly crates inherit from `[workspace.package]` must be present.
for (const key of ['version', 'edition', 'license']) {
  if (!standalonePackage.has(key)) {
    failures.push(`standalone-Cargo.toml [workspace.package] is missing \`${key}\``);
  }
}
for (const key of ['edition', 'rust-version']) {
  const expected = workspacePackage.get(key);
  const actual = standalonePackage.get(key);
  if (expected && actual && expected !== actual) {
    failures.push(
      `standalone-Cargo.toml [workspace.package] \`${key}\` is ${actual}, host workspace pins ${expected}`,
    );
  }
}

// 3. Every `<dep>.workspace = true` in a Fly manifest must resolve in the template, and the pin
//    must match the host workspace so an extracted Fly does not silently build against a
//    different dependency version than the one CI tests here.
const manifests = [
  'crates/ui/fly/Cargo.toml',
  'crates/ui/fly/ui/Cargo.toml',
  'crates/ui/fly/web/Cargo.toml',
  'crates/ui/fly/browser/Cargo.toml',
  'crates/ui/fly/leptos/Cargo.toml',
  'crates/ui/fly/dioxus/Cargo.toml',
];

// Dependencies the standalone template deliberately does not provide.
//
// This used to be a list of "known blockers" that simply suppressed failures. Each entry is now
// an assertion: a dependency may only be absent from the template if the crate declares it
// `optional = true`, so that an extracted build can switch it off. Exempting a mandatory
// dependency would mean the template cannot build, which is exactly what this gate exists to
// prevent.
const standaloneUnavailable = new Set(['rustok-ui-i18n']);

// `[package]` fields also use `<key>.workspace = true`, but they are inherited from
// `[workspace.package]`, which is covered by the checks above.

/// Collect `[workspace.lints.<tool>]` entries as a `tool::lint -> level` map.
function collectLintPolicy(source) {
  const policy = new Map();
  const text = withoutComments(source);
  const sectionPattern = /^\s*\[workspace\.lints\.([A-Za-z0-9_-]+)\]\s*$/gm;
  let match;
  while ((match = sectionPattern.exec(text)) !== null) {
    const tool = match[1];
    const rest = text.slice(match.index + match[0].length);
    const body = rest.split(/^\s*\[/m)[0];
    for (const entry of body.matchAll(/^\s*([A-Za-z0-9_:-]+)\s*=\s*"([a-z]+)"\s*$/gm)) {
      policy.set(`${tool}::${entry[1]}`, entry[2]);
    }
  }
  return policy;
}

const packageFields = new Set([
  'version',
  'edition',
  'license',
  'rust-version',
  'repository',
  'description',
  'authors',
  'homepage',
  'documentation',
  'readme',
  'keywords',
  'categories',
  'publish',
  'exclude',
  'include',
]);

for (const path of manifests) {
  const manifest = await read(path);
  // Cargo accepts both `dep.workspace = true` and `dep = { workspace = true, .. }`; Fly manifests
  // use both spellings.
  const inherited =
    /^\s*([A-Za-z0-9_-]+)\s*(?:\.workspace\s*=\s*true|=\s*\{[^}]*\bworkspace\s*=\s*true)/gm;
  for (const match of withoutComments(manifest).matchAll(inherited)) {
    const dependency = match[1];
    if (packageFields.has(dependency)) continue;
    if (standaloneUnavailable.has(dependency)) {
      // Must be optional, otherwise the extracted workspace cannot build without it.
      const declaration = new RegExp(
        `^\\s*${dependency.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*=\\s*\\{[^}]*\\}`,
        'm',
      ).exec(withoutComments(manifest));
      if (!declaration) {
        failures.push(
          `${path}: \`${dependency}\` is exempt from the standalone template but is not declared as a table, so it cannot be optional`,
        );
      } else if (!/\boptional\s*=\s*true/.test(declaration[0])) {
        failures.push(
          `${path}: \`${dependency}\` is exempt from the standalone template but is not \`optional = true\`; the extracted workspace could not build`,
        );
      }
      continue;
    }
    if (!standaloneDeps.has(dependency)) {
      failures.push(`${path}: \`${dependency}.workspace = true\` has no entry in standalone-Cargo.toml`);
      continue;
    }
    const expected = workspaceDeps.get(dependency);
    const actual = standaloneDeps.get(dependency);
    if (expected && actual && expected !== actual) {
      failures.push(
        `standalone-Cargo.toml pins ${dependency} = ${actual}, host workspace pins ${expected}`,
      );
    }
  }
}

// 3b. Lint policy must be mirrored.
//
// Every Fly crate declares `[lints] workspace = true`, which Cargo rejects outright if the
// workspace root has no `[workspace.lints]`. The dependency regex above does not match that
// spelling, so without this check the template could silently stop building when extracted.
{
  const hostLints = collectLintPolicy(workspace);
  const standaloneLints = collectLintPolicy(standalone);

  for (const path of manifests) {
    const manifest = withoutComments(await read(path));
    if (!/^\s*\[lints\]/m.test(manifest)) continue;
    if (!/^\s*workspace\s*=\s*true/m.test(manifest)) continue;
    if (standaloneLints.size === 0) {
      failures.push(
        `${path}: declares \`[lints] workspace = true\` but standalone-Cargo.toml has no [workspace.lints.*] section`,
      );
      break;
    }
  }

  for (const [lint, level] of hostLints) {
    if (!standaloneLints.has(lint)) {
      failures.push(`standalone-Cargo.toml is missing lint \`${lint}\` enforced by the host workspace`);
    } else if (standaloneLints.get(lint) !== level) {
      failures.push(
        `standalone-Cargo.toml sets ${lint} = "${standaloneLints.get(lint)}", host workspace sets "${level}"`,
      );
    }
  }
  for (const lint of standaloneLints.keys()) {
    if (!hostLints.has(lint)) {
      failures.push(`standalone-Cargo.toml enforces lint \`${lint}\` that the host workspace does not`);
    }
  }
}

// 4. The substitution must stay documented, so nobody extracts Fly without learning what the
//    standalone build gives up (CLDR likely-subtag inference).
for (const marker of ['rustok-ui-i18n', 'LocaleResolver', 'default-features = false']) {
  if (!standalone.includes(marker)) {
    failures.push(
      `standalone-Cargo.toml must keep documenting the locale resolver substitution (missing: ${marker})`,
    );
  }
}

if (failures.length > 0) {
  console.error('Fly standalone workspace template verification failed:');
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log('Fly standalone workspace template is consistent with the host workspace.');
