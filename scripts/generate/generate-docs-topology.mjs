#!/usr/bin/env node
/**
 * Generates and validates documentation sections derived from `modules.toml`.
 * 
 * Target files:
 * - docs/modules/overview.md (module tables)
 * - docs/architecture/diagram.md (Mermaid topology graph)
 * 
 * Usage:
 *   node scripts/generate/generate-docs-topology.mjs         # Updates target files
 *   node scripts/generate/generate-docs-topology.mjs --check # Exits 1 if files are stale
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const check = process.argv.includes('--check');

function read(rel) {
  return fs.readFileSync(path.join(root, rel), 'utf8');
}

function write(rel, content) {
  fs.writeFileSync(path.join(root, rel), content, 'utf8');
}

function parseModulesToml() {
  const raw = read('modules.toml');
  const lines = raw.split(/\r?\n/);
  const core = [];
  const optional = [];
  const extensions = [];

  let insideModules = false;

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;

    const sectionMatch = trimmed.match(/^\[([^\]]+)\]$/);
    if (sectionMatch) {
      insideModules = sectionMatch[1] === 'modules';
      continue;
    }

    if (!insideModules) continue;

    const entryMatch = trimmed.match(/^([a-zA-Z0-9_]+)\s*=\s*\{([^}]+)\}/);
    if (!entryMatch) continue;

    const slug = entryMatch[1];
    const props = entryMatch[2];

    const crateMatch = props.match(/crate\s*=\s*"([^"]+)"/);
    const pathMatch = props.match(/path\s*=\s*"([^"]+)"/);
    const requiredMatch = props.match(/required\s*=\s*(true|false)/);
    const runtimeMatch = props.match(/runtime\s*=\s*"([^"]+)"/);
    const depsMatch = props.match(/depends_on\s*=\s*\[([^\]]*)\]/);

    const crateName = crateMatch ? crateMatch[1] : `rustok-${slug}`;
    const modulePath = pathMatch ? pathMatch[1] : `crates/modules/${crateName}`;
    const isRequired = requiredMatch ? requiredMatch[1] === 'true' : false;
    const runtime = runtimeMatch ? runtimeMatch[1] : 'module';
    const dependsOn = depsMatch
      ? depsMatch[1]
          .split(',')
          .map((d) => d.trim().replace(/^"|"$/g, ''))
          .filter(Boolean)
      : [];

    const item = { slug, crateName, modulePath, isRequired, runtime, dependsOn };

    if (runtime === 'extension') {
      extensions.push(item);
    } else if (isRequired) {
      core.push(item);
    } else {
      optional.push(item);
    }
  }

  return { core, optional, extensions };
}

function renderOverviewTables({ core, optional, extensions }) {
  const lines = [
    '<!-- @generated:module-topology-begin -->',
    '### Core',
    '',
    '| Slug | Crate | Depends on |',
    '|---|---|---|',
  ];

  for (const mod of core) {
    const deps = mod.dependsOn.length > 0 ? mod.dependsOn.map((d) => `\`${d}\``).join(', ') : '—';
    lines.push(`| \`${mod.slug}\` | \`${mod.crateName}\` | ${deps} |`);
  }

  lines.push('');
  lines.push('### Optional');
  lines.push('');
  lines.push('| Slug | Crate | Depends on |');
  lines.push('|---|---|---|');

  for (const mod of optional) {
    const deps = mod.dependsOn.length > 0 ? mod.dependsOn.map((d) => `\`${d}\``).join(', ') : '—';
    lines.push(`| \`${mod.slug}\` | \`${mod.crateName}\` | ${deps} |`);
  }

  lines.push('');
  lines.push('### Capability Extensions');
  lines.push('');
  lines.push('| Slug | Crate | Runtime |');
  lines.push('|---|---|---|');

  for (const mod of extensions) {
    lines.push(`| \`${mod.slug}\` | \`${mod.crateName}\` | \`${mod.runtime}\` |`);
  }

  lines.push('<!-- @generated:module-topology-end -->');
  return lines.join('\n');
}

function renderMermaidDiagram({ core, optional, extensions }) {
  const lines = [
    '<!-- @generated:architecture-diagram-begin -->',
    '```mermaid',
    'graph TD',
    '    subgraph Hosts["Host applications"]',
    '        SERVER["apps/server (Axum composition root)"]',
    '        ADMIN["apps/admin (Leptos)"]',
    '        STOREFRONT["apps/storefront (Leptos)"]',
    '        NEXT_ADMIN["apps/next-admin (Next.js)"]',
    '        NEXT_FRONT["apps/next-frontend (Next.js)"]',
    '    end',
    '',
    '    subgraph Core["Core platform modules (required = true)"]',
  ];

  for (const mod of core) {
    const node = `CORE_${mod.slug.toUpperCase().replace(/[^A-Z0-9_]/g, '_')}`;
    lines.push(`        ${node}["${mod.slug} (${mod.crateName})"]`);
  }
  lines.push('    end', '');

  lines.push('    subgraph Optional["Optional domain modules (tenant-managed)"]');
  for (const mod of optional) {
    const node = `OPT_${mod.slug.toUpperCase().replace(/[^A-Z0-9_]/g, '_')}`;
    lines.push(`        ${node}["${mod.slug}"]`);
  }
  lines.push('    end', '');

  lines.push('    subgraph Extensions["Capability extensions (runtime = \\"extension\\")"]');
  for (const mod of extensions) {
    const node = `EXT_${mod.slug.toUpperCase().replace(/[^A-Z0-9_]/g, '_')}`;
    lines.push(`        ${node}["${mod.slug} (${mod.crateName})"]`);
  }
  lines.push('    end', '');

  lines.push('    subgraph Foundations["Platform foundation & shared libraries"]');
  lines.push('        CORE_LIB["rustok-core"]');
  lines.push('        API_LIB["rustok-api"]');
  lines.push('        EVENTS_LIB["rustok-events"]');
  lines.push('        RUNTIME_LIB["rustok-runtime"]');
  lines.push('        WEB_LIB["rustok-web"]');
  lines.push('        STORAGE_LIB["rustok-storage"]');
  lines.push('        TELEMETRY_LIB["rustok-telemetry"]');
  lines.push('        FBA_LIB["rustok-fba"]');
  lines.push('        MCP_LIB["rustok-mcp"]');
  lines.push('    end', '');

  lines.push('    SERVER --> Core');
  lines.push('    SERVER --> Optional');
  lines.push('    SERVER --> Extensions');
  lines.push('    Core --> Foundations');
  lines.push('    Optional --> Foundations');
  lines.push('    Extensions --> Foundations');
  lines.push('    ADMIN --> Core');
  lines.push('    STOREFRONT --> Core');
  lines.push('```');
  lines.push('<!-- @generated:architecture-diagram-end -->');

  return lines.join('\n');
}

function updateGeneratedBlock(content, beginMarker, endMarker, newBlock) {
  const startIndex = content.indexOf(beginMarker);
  const endIndex = content.indexOf(endMarker);

  if (startIndex === -1 || endIndex === -1 || endIndex < startIndex) {
    return null;
  }

  const before = content.slice(0, startIndex);
  const after = content.slice(endIndex + endMarker.length);
  return before + newBlock + after;
}

export function runGenerator({ checkMode = false } = {}) {
  const modules = parseModulesToml();
  let hasDrift = false;

  // 1. Update docs/modules/overview.md
  const overviewRel = 'docs/modules/overview.md';
  const overviewContent = read(overviewRel);
  const overviewBlock = renderOverviewTables(modules);
  const updatedOverview = updateGeneratedBlock(
    overviewContent,
    '<!-- @generated:module-topology-begin -->',
    '<!-- @generated:module-topology-end -->',
    overviewBlock
  );

  if (updatedOverview === null) {
    console.error(`Marker <!-- @generated:module-topology-begin --> not found in ${overviewRel}`);
    hasDrift = true;
  } else if (updatedOverview !== overviewContent) {
    if (checkMode) {
      console.error(`[STALE] ${overviewRel} module tables are out of date with modules.toml`);
      hasDrift = true;
    } else {
      write(overviewRel, updatedOverview);
      console.log(`[UPDATED] ${overviewRel}`);
    }
  }

  // 2. Update docs/architecture/diagram.md
  const diagramRel = 'docs/architecture/diagram.md';
  const diagramContent = read(diagramRel);
  const diagramBlock = renderMermaidDiagram(modules);
  const updatedDiagram = updateGeneratedBlock(
    diagramContent,
    '<!-- @generated:architecture-diagram-begin -->',
    '<!-- @generated:architecture-diagram-end -->',
    diagramBlock
  );

  if (updatedDiagram === null) {
    console.error(`Marker <!-- @generated:architecture-diagram-begin --> not found in ${diagramRel}`);
    hasDrift = true;
  } else if (updatedDiagram !== diagramContent) {
    if (checkMode) {
      console.error(`[STALE] ${diagramRel} architecture diagram is out of date with modules.toml`);
      hasDrift = true;
    } else {
      write(diagramRel, updatedDiagram);
      console.log(`[UPDATED] ${diagramRel}`);
    }
  }

  if (checkMode && hasDrift) {
    process.exit(1);
  }

  if (!hasDrift && checkMode) {
    console.log('[OK] Module documentation topology is up to date.');
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  runGenerator({ checkMode: check });
}
