#!/usr/bin/env node
/**
 * Canonical Documentation System Gate: `verify:docs`
 * 
 * Verifies that documentation across RusToK is self-proving, up-to-date,
 * syntactically valid, free of broken links and obsolete terms, and properly governed.
 * 
 * Gates executed:
 * 1. ADR governance (verify-adrs.mjs)
 * 2. Manifest/Topology synchronization (generate-docs-topology.mjs --check)
 * 3. Malformed literal escapes (\n, \t) detection in markdown text
 * 4. Internal relative links and anchor validation across platform entrypoints
 * 5. Forbidden stale terminology guard
 * 6. Canonical metadata & status schema validation
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execSync } from 'node:child_process';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '../..');

const errors = [];
function fail(msg) {
  errors.push(msg);
}

function readText(relPath) {
  return fs.readFileSync(path.join(repoRoot, relPath), 'utf8');
}

function exists(relPath) {
  return fs.existsSync(path.join(repoRoot, relPath));
}

// Core governed documents subject to strict link and metadata checks
const governedDocs = [
  'docs/index.md',
  'ARCHITECTURE.md',
  'docs/AI_CONTEXT.md',
  'docs/glossary.md',
  'docs/architecture/overview.md',
  'docs/architecture/principles.md',
  'docs/architecture/diagram.md',
  'docs/modules/overview.md',
  'docs/verification/README.md',
  'README.md',
];

console.log('[1/6] Verifying ADR governance...');
try {
  execSync('node scripts/verify/verify-adrs.mjs', {
    cwd: repoRoot,
    stdio: 'pipe',
    encoding: 'utf8',
  });
} catch (e) {
  fail(`ADR governance verification failed:\n${e.stderr || e.stdout || e.message}`);
}

console.log('[2/6] Verifying module topology & architecture diagrams synchronization...');
try {
  execSync('node scripts/generate/generate-docs-topology.mjs --check', {
    cwd: repoRoot,
    stdio: 'pipe',
    encoding: 'utf8',
  });
} catch (e) {
  fail(`Module topology verification failed. Run node scripts/generate/generate-docs-topology.mjs to update.\n${e.stderr || e.stdout || e.message}`);
}

console.log('[3/6] Checking for malformed literal escape characters in markdown text...');
for (const rel of governedDocs) {
  if (!exists(rel)) continue;
  const content = readText(rel);
  // Match unescaped literal '\n' or '\t' outside code blocks
  // Check lines outside ``` blocks
  const lines = content.split(/\r?\n/);
  let inCodeBlock = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.trim().startsWith('```')) {
      inCodeBlock = !inCodeBlock;
      continue;
    }
    if (inCodeBlock) continue;

    // Detect literal literal \n or \t followed by list hyphen or bracket, e.g. "foo\n- bar"
    if (/\\n\s*[-*[]|\\t\s*[-*[]/.test(line)) {
      fail(`${rel}:${i + 1}: contains malformed unescaped literal escape sequence: ${line.trim()}`);
    }
  }
}

console.log('[4/6] Verifying internal relative links across docs/ and anchors in governed documents...');
function slugifyHeading(text) {
  return text
    .toLowerCase()
    .trim()
    .replace(/[^\w\s-]/g, '')
    .replace(/\s+/g, '-');
}

function extractHeadings(markdown) {
  const headings = new Set();
  for (const line of markdown.split(/\r?\n/)) {
    const match = line.match(/^#{1,6}\s+(.+)$/);
    if (match) {
      headings.add(slugifyHeading(match[1]));
    }
  }
  return headings;
}

// Collect all docs files
const allDocsFiles = [];
function collectDocsFiles(dir) {
  for (const f of fs.readdirSync(dir, { withFileTypes: true })) {
    if (['node_modules', 'target', '.git', 'dist', '.next'].includes(f.name)) continue;
    const full = path.join(dir, f.name);
    if (f.isDirectory()) collectDocsFiles(full);
    else if (f.name.endsWith('.md')) allDocsFiles.push(full);
  }
}
collectDocsFiles(path.join(repoRoot, 'docs'));
governedDocs.forEach((d) => {
  const full = path.join(repoRoot, d);
  if (!allDocsFiles.includes(full) && fs.existsSync(full)) allDocsFiles.push(full);
});

for (const fullPath of allDocsFiles) {
  const rel = path.relative(repoRoot, fullPath).replace(/\\/g, '/');
  const content = fs.readFileSync(fullPath, 'utf8');
  const dir = path.dirname(fullPath);
  const isGoverned = governedDocs.includes(rel);

  const lines = content.split(/\r?\n/);
  let inCodeBlock = false;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.trim().startsWith('```')) {
      inCodeBlock = !inCodeBlock;
      continue;
    }
    if (inCodeBlock) continue;

    const linkRegex = /\[([^\]]+)\]\(([^)]+)\)/g;
    let match;
    while ((match = linkRegex.exec(line))) {
      const rawTarget = match[2].trim();
      if (/^(https?:\/\/|mailto:)/i.test(rawTarget)) continue;

      const [targetPath, anchor] = rawTarget.split('#');
      let resolvedFile;
      if (!targetPath) {
        resolvedFile = fullPath;
      } else {
        resolvedFile = path.resolve(dir, targetPath);
      }

      if (!fs.existsSync(resolvedFile)) {
        fail(`${rel}:${i + 1}: broken link to missing file: "${rawTarget}" (resolved: "${path.relative(repoRoot, resolvedFile)}")`);
        continue;
      }

      if (isGoverned && anchor && resolvedFile.endsWith('.md')) {
        const targetMarkdown = fs.readFileSync(resolvedFile, 'utf8');
        const headings = extractHeadings(targetMarkdown);
        const targetSlug = slugifyHeading(decodeURIComponent(anchor));
        if (!headings.has(targetSlug)) {
          const idRegex = new RegExp(`id=["']${targetSlug}["']`, 'i');
          const nameRegex = new RegExp(`name=["']${targetSlug}["']`, 'i');
          if (!idRegex.test(targetMarkdown) && !nameRegex.test(targetMarkdown)) {
            fail(`${rel}:${i + 1}: broken anchor "#${anchor}" in ${path.relative(repoRoot, resolvedFile)}`);
          }
        }
      }
    }
  }
}

console.log('[5/6] Checking for forbidden stale terminology...');
const forbiddenPatterns = [
  {
    regex: /Do not invent a third module type besides Core and Optional/i,
    reason: 'Violates platform contract; runtime = "extension" is a valid deployment composition mode',
  },
  {
    regex: /tenant context.*enforced at compile-time and embedded in composite database keys/i,
    reason: 'Stale wording; multi-tenancy is enforced via composite DB keys and runtime PortContext, not solely compile-time',
  },
  {
    regex: /\b(only \/api\/fn|only \/api\/graphql)\b/i,
    reason: 'Conflicting transport wording; RusToK supports dual #[server] and GraphQL/REST surfaces',
  },
];

for (const rel of governedDocs) {
  if (!exists(rel)) continue;
  const content = readText(rel);
  const lines = content.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    for (const { regex, reason } of forbiddenPatterns) {
      if (regex.test(line)) {
        fail(`${rel}:${i + 1}: forbidden stale terminology: "${line.trim()}" (${reason})`);
      }
    }
  }
}

console.log('[6/6] Verifying canonical metadata and status schema on central documents...');
const validDocTypes = new Set([
  'current_contract',
  'accepted_target',
  'implementation_plan',
  'runbook',
  'research',
  'evidence',
  'generated_reference',
  'historical',
]);

const validStatuses = new Set([
  'current',
  'accepted_target',
  'in_progress',
  'superseded',
  'archived',
]);

const metadataGovernedDocs = [
  'docs/index.md',
  'docs/AI_CONTEXT.md',
  'docs/glossary.md',
  'docs/architecture/overview.md',
  'docs/architecture/principles.md',
  'docs/architecture/diagram.md',
  'docs/modules/overview.md',
  'docs/verification/README.md',
];

for (const rel of metadataGovernedDocs) {
  if (!exists(rel)) continue;
  const content = readText(rel);
  if (!content.startsWith('---')) {
    fail(`${rel}: missing YAML front matter`);
    continue;
  }

  const frontMatterMatch = content.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!frontMatterMatch) {
    fail(`${rel}: malformed YAML front matter`);
    continue;
  }

  const fmLines = frontMatterMatch[1].split(/\r?\n/);
  const fm = {};
  for (const line of fmLines) {
    const m = line.match(/^([a-zA-Z_]+):\s*(.+)$/);
    if (m) {
      fm[m[1]] = m[2].trim();
    }
  }

  // Check doc_type
  if (!fm.doc_type) {
    fail(`${rel}: missing "doc_type" in front matter (must be one of: ${[...validDocTypes].join(', ')})`);
  } else if (!validDocTypes.has(fm.doc_type)) {
    fail(`${rel}: invalid doc_type "${fm.doc_type}" (must be one of: ${[...validDocTypes].join(', ')})`);
  }

  // Check status
  if (!fm.status) {
    fail(`${rel}: missing "status" in front matter (must be one of: ${[...validStatuses].join(', ')})`);
  } else if (!validStatuses.has(fm.status)) {
    fail(`${rel}: invalid status "${fm.status}" (must be one of: ${[...validStatuses].join(', ')})`);
  }

  // Reject fake snapshot and ungrounded verified status
  if (fm.last_verified_snapshot) {
    fail(`${rel}: forbidden "last_verified_snapshot" in front matter; use canonical status and owner tracking`);
  }
  if (fm.status === 'verified') {
    fail(`${rel}: forbidden unverified "status: verified"; use "status: current" with verified gate`);
  }
  if (fm.kind === 'project_overview') {
    fail(`${rel}: deprecated "kind: project_overview"; replace with canonical "doc_type"`);
  }
}

console.log('');
if (errors.length > 0) {
  console.error(`Documentation governance verification failed with ${errors.length} error(s):`);
  for (const err of errors) {
    console.error(`  - ${err}`);
  }
  process.exit(1);
}

console.log(`Documentation governance verification passed successfully!`);
console.log(`- ${governedDocs.length} core platform documents verified`);
console.log(`- All internal links and anchors resolved`);
console.log(`- All module topology and Mermaid diagrams up to date`);
console.log(`- Zero forbidden stale terminology or unescaped escape sequences detected`);
