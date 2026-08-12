#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, '../..');
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), 'utf8');
}

function readJson(relativePath) {
  return JSON.parse(readRepo(relativePath));
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath) {
  if (!existsSync(repoPath(relativePath))) fail(`${relativePath}: missing required file`);
}

function assertContains(text, marker, label) {
  if (!text.includes(marker)) fail(`${label}: missing ${marker}`);
}

function assertNotContains(text, marker, label) {
  if (text.includes(marker)) fail(`${label}: forbidden ${marker}`);
}

function sameList(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-ai-richtext-boundary.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan.md';
const registryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';
const selfTestPath = 'scripts/verify/verify-blog-ai-richtext-boundary.test.mjs';
const packagePath = 'package.json';

for (const requiredPath of [
  evidencePath,
  planPath,
  registryPath,
  selfTestPath,
  packagePath,
]) {
  assertExists(requiredPath);
}

const evidence = readJson(evidencePath);
if (
  evidence.schema_version !== 1 ||
  evidence.module !== 'blog' ||
  evidence.surface !== 'ai_blog_draft_richtext_boundary'
) {
  fail('evidence identity drift');
}
if (
  evidence.status !== 'source_verified_no_compile' ||
  evidence.compile_policy !== 'not_run_by_request'
) {
  fail('evidence status drift');
}
if (
  evidence.guardrail !== 'scripts/verify/verify-blog-ai-richtext-boundary.mjs' ||
  evidence.guardrail_test !== selfTestPath
) {
  fail('evidence guardrail path drift');
}

const shimPath = evidence.shim?.path;
const writerPath = evidence.writer?.path;
if (shimPath !== 'crates/rustok-ai/src/rustok_blog.rs') fail('evidence shim path drift');
if (writerPath !== 'crates/rustok-ai/src/direct.rs') fail('evidence writer path drift');
for (const sourcePath of [shimPath, writerPath]) {
  if (sourcePath) assertExists(sourcePath);
}

if (shimPath && existsSync(repoPath(shimPath))) {
  const shim = readRepo(shimPath);
  const productionShim = shim.split('#[cfg(test)]', 1)[0];
  const reexport = shim.match(/pub use rustok_blog_owner::\{([\s\S]*?)\};/);
  if (!reexport) {
    fail(`${shimPath}: missing rustok_blog_owner re-export`);
  } else {
    const actualReexports = reexport[1]
      .split(',')
      .map((value) => value.trim())
      .filter(Boolean);
    const expectedReexports = evidence.shim?.allowed_reexports ?? [];
    if (!sameList(actualReexports, expectedReexports)) {
      fail(
        `${shimPath}: owner re-export drift; expected ${expectedReexports.join('|')}, got ${actualReexports.join('|')}`,
      );
    }
  }
  for (const forbidden of evidence.shim?.forbidden_reexports ?? []) {
    if (new RegExp(`\\b${forbidden}\\b`).test(productionShim)) {
      fail(`${shimPath}: forbidden owner re-export ${forbidden}`);
    }
  }
}

if (writerPath && existsSync(repoPath(writerPath))) {
  const writer = readRepo(writerPath);
  const productionWriter = writer.split('#[cfg(test)]', 1)[0];
  for (const marker of evidence.writer?.required_markers ?? []) {
    assertContains(productionWriter, marker, `${writerPath}: canonical AI Blog draft writer`);
  }
  for (const marker of evidence.writer?.forbidden_markers ?? []) {
    assertNotContains(productionWriter, marker, `${writerPath}: canonical AI Blog draft writer`);
  }
}

const packageJson = readJson(packagePath);
if (
  packageJson.scripts?.['verify:blog:ai-richtext-boundary'] !==
  'node scripts/verify/verify-blog-ai-richtext-boundary.mjs'
) {
  fail('package verifier command drift');
}
if (
  packageJson.scripts?.['test:verify:blog:ai-richtext-boundary'] !==
  `node ${selfTestPath}`
) {
  fail('package self-test command drift');
}
if (
  !packageJson.scripts?.['verify:blog:fba']?.includes(
    'verify:blog:ai-richtext-boundary',
  )
) {
  fail('Blog FBA aggregate does not include AI richtext boundary verifier');
}
if (
  !packageJson.scripts?.['test:verify:blog:fba']?.includes(
    'test:verify:blog:ai-richtext-boundary',
  )
) {
  fail('Blog FBA self-test aggregate does not include AI richtext boundary fixture');
}

const registry = readJson(registryPath);
const gate = registry.verification_chain?.source_gates?.ai_richtext_boundary;
if (
  registry.schema_version !== 13 ||
  gate?.package_script !== 'verify:blog:ai-richtext-boundary' ||
  gate?.test_package_script !== 'test:verify:blog:ai-richtext-boundary' ||
  gate?.verifier !== 'scripts/verify/verify-blog-ai-richtext-boundary.mjs' ||
  gate?.self_test !== selfTestPath ||
  gate?.evidence !== evidencePath
) {
  fail('Blog FBA registry AI richtext source-gate drift');
}

const plan = readRepo(planPath);
for (const marker of [
  evidencePath,
  'scripts/verify/verify-blog-ai-richtext-boundary.mjs',
  'scripts/verify/verify-blog-ai-richtext-boundary.test.mjs',
  'AI Blog owner shim',
  '35.',
]) {
  assertContains(plan, marker, `${planPath}: audited AI richtext plan`);
}
if (failures.length > 0) {
  console.error('Blog AI richtext boundary verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  '[verify-blog-ai-richtext-boundary] AI Blog drafts retain a canonical owner-only richtext boundary',
);
