#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const generatedRel = 'crates/rustok-cli-registry/src/generated.rs';
const registryCargoRel = 'crates/rustok-cli-registry/Cargo.toml';
const check = process.argv.includes('--check');

function read(rel) {
  return fs.readFileSync(path.join(root, rel), 'utf8');
}

function exists(rel) {
  return fs.existsSync(path.join(root, rel));
}

function parseModuleEntries() {
  const raw = read('modules.toml');
  const entries = [];
  const moduleRe = /^([A-Za-z0-9_]+)\s*=\s*\{([^\n]+)\}/gm;
  let match;
  while ((match = moduleRe.exec(raw))) {
    const slug = match[1];
    const body = match[2];
    const crateMatch = body.match(/crate\s*=\s*"([^"]+)"/);
    const pathMatch = body.match(/path\s*=\s*"([^"]+)"/);
    if (crateMatch && pathMatch) {
      entries.push({ slug, crateName: crateMatch[1], modulePath: pathMatch[1] });
    }
  }
  return entries.sort((left, right) => left.slug.localeCompare(right.slug));
}

function section(raw, name) {
  const lines = raw.split(/\r?\n/);
  const sectionLines = [];
  let inside = false;

  for (const line of lines) {
    const header = line.match(/^\s*\[([^\]]+)\]\s*$/);
    if (header) {
      if (inside) break;
      inside = header[1] === name;
      continue;
    }
    if (inside) sectionLines.push(line);
  }

  return sectionLines.join('\n');
}

function value(sectionRaw, key) {
  const match = sectionRaw.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"\\s*$`, 'm'));
  return match ? match[1] : null;
}

function parseRootProviders() {
  if (!exists('cli-registry.toml')) return [];

  const raw = read('cli-registry.toml');
  const providers = [];
  let current = null;

  for (const line of raw.split(/\r?\n/)) {
    const header = line.match(/^\s*\[providers\.([A-Za-z0-9_]+)\]\s*$/);
    if (header) {
      current = { slug: header[1], source: 'cli-registry.toml' };
      providers.push(current);
      continue;
    }
    if (!current || line.trim().startsWith('#')) continue;

    const pair = line.match(/^\s*([A-Za-z0-9_]+)\s*=\s*"([^"]+)"\s*$/);
    if (pair) current[pair[1]] = pair[2];
  }

  return providers.map(normalizeProvider);
}

function normalizeProvider(provider) {
  const namespace = provider.namespace ?? provider.slug;
  const factory = provider.factory;
  const providerType = provider.provider;
  const source = provider.source ?? provider.slug;

  if (!factory && !providerType) {
    throw new Error(`${source}: CLI provider must declare factory or provider`);
  }
  if (factory && providerType) {
    throw new Error(`${source}: CLI provider must declare only one of factory or provider`);
  }
  if (!/^[a-z][a-z0-9_]*$/.test(namespace)) {
    throw new Error(`${source}: CLI provider namespace must be snake_case`);
  }
  if (factory && !/^[A-Za-z_][A-Za-z0-9_:]*$/.test(factory)) {
    throw new Error(`${source}: CLI provider factory must be a Rust path`);
  }
  if (providerType && !/^[A-Za-z_][A-Za-z0-9_:]*$/.test(providerType)) {
    throw new Error(`${source}: CLI provider provider type must be a Rust path`);
  }

  return {
    slug: provider.slug,
    namespace,
    factory,
    provider: providerType,
    crateName: provider.crateName ?? rustPathCrateName(factory ?? providerType),
  };
}

function rustPathCrateName(rustPath) {
  const cratePath = rustPath.split('::')[0];
  return cratePath.replaceAll('_', '-');
}

function validateRegistryDependencies(providers) {
  const registryCargo = read(registryCargoRel);
  const missing = providers
    .map((provider) => provider.crateName)
    .filter((crateName) => crateName && crateName !== 'rustok-cli-core')
    .filter((crateName, index, all) => all.indexOf(crateName) === index)
    .filter((crateName) => !registryCargo.includes(`${crateName}.workspace = true`));

  if (missing.length > 0) {
    throw new Error(`${registryCargoRel}: missing selected provider dependencies: ${missing.join(', ')}`);
  }
}

function parseCliProviders() {
  const providers = parseRootProviders();
  for (const entry of parseModuleEntries()) {
    const manifestRel = `${entry.modulePath}/rustok-module.toml`;
    if (!exists(manifestRel)) continue;

    const cliSection = section(read(manifestRel), 'provides.cli');
    if (!cliSection) continue;

    providers.push(
      normalizeProvider({
        slug: entry.slug,
        source: `${manifestRel}: [provides.cli]`,
        namespace: value(cliSection, 'namespace') ?? entry.slug,
        factory: value(cliSection, 'factory'),
        provider: value(cliSection, 'provider'),
      }),
    );
  }

  return providers.sort((left, right) => left.namespace.localeCompare(right.namespace));
}

function renderProvider(provider) {
  if (provider.factory) return `        ${provider.factory}(),`;
  return `        Box::new(${provider.provider}::default()),`;
}

function renderGenerated(providers) {
  const lines = [
    '// @generated by scripts/generate/generate-cli-registry.mjs',
    '// Do not edit by hand.',
    '',
    'use rustok_cli_core::CommandProvider;',
    '',
    'pub fn generated_providers() -> Vec<Box<dyn CommandProvider>> {',
  ];

  if (providers.length === 0) {
    lines.push('    Vec::new()');
  } else {
    lines.push('    vec![');
    for (const provider of providers) {
      lines.push(`        // ${provider.slug} / ${provider.namespace}`);
      lines.push(renderProvider(provider));
    }
    lines.push('    ]');
  }

  lines.push('}', '');
  return lines.join('\n');
}

const providers = parseCliProviders();
validateRegistryDependencies(providers);
const next = renderGenerated(providers);
const target = path.join(root, generatedRel);

if (check) {
  const current = exists(generatedRel) ? read(generatedRel) : '';
  if (current !== next) {
    console.error(`${generatedRel} is stale. Run node scripts/generate/generate-cli-registry.mjs`);
    process.exit(1);
  }
  console.log('rustok-cli-registry generated source is up to date');
} else {
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, next);
  console.log(`wrote ${generatedRel}`);
}
