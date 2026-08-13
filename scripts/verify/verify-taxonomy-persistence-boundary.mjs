#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';

const repoRoot = path.resolve(
  process.env.RUSTOK_TAXONOMY_BOUNDARY_ROOT || process.cwd(),
);
const cratesRoot = path.join(repoRoot, 'crates');
const violations = [];

const persistenceModelPattern =
  /\btaxonomy_term(?:_translation|_alias|_route_key)?::(?:Entity|ActiveModel|Column|Model)\b/g;
const directEntitiesModulePattern = /\brustok_taxonomy\s*::\s*entities\b/g;
const nestedEntitiesImportPattern =
  /use\s+rustok_taxonomy\s*::\s*\{[^;]*\bentities\s*::/gs;
const cfgAttributePattern = /#\s*\[\s*cfg\s*\((.*?)\)\s*\]/gs;

function normalize(relativePath) {
  return relativePath.split(path.sep).join('/');
}

function isExcluded(relativePath) {
  const normalized = normalize(relativePath);
  if (normalized.startsWith('crates/rustok-taxonomy/')) return true;
  if (normalized.includes('/src/migrations/')) return true;
  if (normalized.includes('/src/entities/')) return true;
  if (normalized.includes('/tests/')) return true;
  const basename = path.posix.basename(normalized);
  return basename.endsWith('_test.rs') || basename.endsWith('_tests.rs');
}

function blankRange(chars, start, end) {
  for (let index = start; index < end; index += 1) {
    if (chars[index] !== '\n') chars[index] = ' ';
  }
}

function skipLineComment(source, start) {
  const newline = source.indexOf('\n', start + 2);
  return newline < 0 ? source.length : newline;
}

function skipBlockComment(source, start) {
  let depth = 1;
  let index = start + 2;
  while (index < source.length && depth > 0) {
    if (source.startsWith('/*', index)) {
      depth += 1;
      index += 2;
    } else if (source.startsWith('*/', index)) {
      depth -= 1;
      index += 2;
    } else {
      index += 1;
    }
  }
  return index;
}

function skipQuotedString(source, start) {
  let index = start + 1;
  while (index < source.length) {
    if (source[index] === '\\') {
      index += 2;
      continue;
    }
    if (source[index] === '"') return index + 1;
    index += 1;
  }
  return source.length;
}

function rawStringEnd(source, start) {
  let index = start;
  if (source[index] === 'b' || source[index] === 'c') index += 1;
  if (source[index] !== 'r') return null;
  index += 1;
  let hashes = '';
  while (source[index] === '#') {
    hashes += '#';
    index += 1;
  }
  if (source[index] !== '"') return null;
  const terminator = `"${hashes}`;
  const end = source.indexOf(terminator, index + 1);
  return end < 0 ? source.length : end + terminator.length;
}

function charLiteralEnd(source, start) {
  const match = source.slice(start).match(
    /^'(?:\\(?:x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f_]+\}|.)|[^'\\\n])'/u,
  );
  return match ? start + match[0].length : null;
}

function maskNonCodeLexemes(source) {
  const chars = source.split('');
  let index = 0;
  while (index < source.length) {
    if (source.startsWith('//', index)) {
      const end = skipLineComment(source, index);
      blankRange(chars, index, end);
      index = end;
      continue;
    }
    if (source.startsWith('/*', index)) {
      const end = skipBlockComment(source, index);
      blankRange(chars, index, end);
      index = end;
      continue;
    }
    const rawEnd = rawStringEnd(source, index);
    if (rawEnd !== null) {
      blankRange(chars, index, rawEnd);
      index = rawEnd;
      continue;
    }
    if (
      (source[index] === 'b' || source[index] === 'c') &&
      source[index + 1] === '"'
    ) {
      const end = skipQuotedString(source, index + 1);
      blankRange(chars, index, end);
      index = end;
      continue;
    }
    if (source[index] === '"') {
      const end = skipQuotedString(source, index);
      blankRange(chars, index, end);
      index = end;
      continue;
    }
    if (source[index] === "'") {
      const end = charLiteralEnd(source, index);
      if (end !== null) {
        blankRange(chars, index, end);
        index = end;
        continue;
      }
    }
    index += 1;
  }
  return chars.join('');
}

function splitTopLevelArgs(expression) {
  const args = [];
  let depth = 0;
  let start = 0;
  for (let index = 0; index < expression.length; index += 1) {
    const char = expression[index];
    if (char === '(') depth += 1;
    if (char === ')') depth = Math.max(0, depth - 1);
    if (char === ',' && depth === 0) {
      args.push(expression.slice(start, index).trim());
      start = index + 1;
    }
  }
  args.push(expression.slice(start).trim());
  return args.filter(Boolean);
}

function cfgFunction(expression, name) {
  const trimmed = expression.trim();
  const prefix = `${name}(`;
  if (!trimmed.startsWith(prefix) || !trimmed.endsWith(')')) return null;
  return splitTopLevelArgs(trimmed.slice(prefix.length, -1));
}

function cfgImpliesTest(expression) {
  const trimmed = expression.trim();
  if (trimmed === 'test') return true;
  const allArgs = cfgFunction(trimmed, 'all');
  if (allArgs) return allArgs.some(cfgImpliesTest);
  const anyArgs = cfgFunction(trimmed, 'any');
  if (anyArgs) return anyArgs.length > 0 && anyArgs.every(cfgImpliesTest);
  return false;
}

function findBalancedBlockEnd(source, openBrace) {
  const code = maskNonCodeLexemes(source);
  let depth = 0;
  for (let index = openBrace; index < code.length; index += 1) {
    if (code[index] === '{') depth += 1;
    if (code[index] === '}') {
      depth -= 1;
      if (depth === 0) return index + 1;
    }
  }
  return source.length;
}

function findTestItemEnd(source, start) {
  const code = maskNonCodeLexemes(source);
  let squareDepth = 0;
  let parenDepth = 0;
  for (let index = start; index < code.length; index += 1) {
    const char = code[index];
    if (char === '[') squareDepth += 1;
    if (char === ']') squareDepth = Math.max(0, squareDepth - 1);
    if (char === '(') parenDepth += 1;
    if (char === ')') parenDepth = Math.max(0, parenDepth - 1);
    if (squareDepth === 0 && parenDepth === 0) {
      if (char === ';') return index + 1;
      if (char === '{') return findBalancedBlockEnd(source, index);
    }
  }
  return source.length;
}

function maskTestOnlyItems(source) {
  const code = maskNonCodeLexemes(source);
  const chars = source.split('');
  cfgAttributePattern.lastIndex = 0;
  for (const match of code.matchAll(cfgAttributePattern)) {
    if (!cfgImpliesTest(match[1])) continue;
    const start = match.index ?? 0;
    const end = findTestItemEnd(source, start + match[0].length);
    blankRange(chars, start, end);
  }
  return chars.join('');
}

function lineNumber(source, offset) {
  return source.slice(0, offset).split('\n').length;
}

function recordMatches(relativePath, source, regex, reason) {
  regex.lastIndex = 0;
  for (const match of source.matchAll(regex)) {
    violations.push({
      path: normalize(relativePath),
      line: lineNumber(source, match.index ?? 0),
      reason,
      token: match[0].replace(/\s+/g, ' ').trim(),
    });
  }
}

function scanFile(absolutePath) {
  const relativePath = path.relative(repoRoot, absolutePath);
  if (isExcluded(relativePath)) return;
  const rawSource = fs.readFileSync(absolutePath, 'utf8');
  const source = maskNonCodeLexemes(maskTestOnlyItems(rawSource));
  recordMatches(
    relativePath,
    source,
    persistenceModelPattern,
    'production consumer accesses a Taxonomy persistence model directly',
  );
  recordMatches(
    relativePath,
    source,
    directEntitiesModulePattern,
    'production consumer imports the Taxonomy persistence entities module directly',
  );
  recordMatches(
    relativePath,
    source,
    nestedEntitiesImportPattern,
    'production consumer imports Taxonomy persistence entities through a nested use',
  );
}

function walk(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      walk(absolutePath);
    } else if (entry.isFile() && entry.name.endsWith('.rs')) {
      scanFile(absolutePath);
    }
  }
}

if (!fs.existsSync(cratesRoot)) {
  console.error(
    `[verify-taxonomy-persistence-boundary] missing crates directory under ${repoRoot}`,
  );
  process.exit(2);
}

walk(cratesRoot);

if (violations.length > 0) {
  console.error('[verify-taxonomy-persistence-boundary] FAIL');
  for (const violation of violations) {
    console.error(
      `- ${violation.path}:${violation.line}: ${violation.reason}: ${violation.token}`,
    );
  }
  console.error(
    'Use a Taxonomy service/owner boundary instead. Persistence access is allowed only inside rustok-taxonomy, retained migrations, entity relation metadata, and tests.',
  );
  process.exit(1);
}

console.log(
  '[verify-taxonomy-persistence-boundary] PASS production consumers do not access Taxonomy SeaORM persistence models directly',
);
