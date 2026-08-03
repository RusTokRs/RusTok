#!/usr/bin/env node

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const crateRoot = path.join(root, 'crates/rustok-build-source');
const manifest = fs.readFileSync(path.join(crateRoot, 'Cargo.toml'), 'utf8');
const reader = fs.readFileSync(path.join(crateRoot, 'src/lib.rs'), 'utf8');
const writer = fs.readFileSync(path.join(crateRoot, 'src/writer.rs'), 'utf8');
const cliManifest = fs.readFileSync(
  path.join(root, 'crates/rustok-modules/cli/Cargo.toml'),
  'utf8',
);
const cli = fs.readFileSync(path.join(root, 'crates/rustok-modules/cli/src/lib.rs'), 'utf8');

for (const marker of [
  'pub struct SourceArchiveBuilder',
  '.create_new(true)',
  'files.sort_by(',
  'write_octal(&mut header[136..148], 0)',
  'header[257..263].copy_from_slice(b"ustar\\0")',
  'FINAL_DESCRIPTOR_FILE | ".cargo/config" | ".cargo/config.toml"',
  'IGNORED_ROOT_PATHS: &[&str] = &[".git", "target"]',
  'write_bounded(',
  'hash_archive(destination)',
  'deterministic_archive_round_trips_through_the_strict_materializer',
]) {
  assert.ok(writer.includes(marker), `canonical source writer is missing ${marker}`);
}

for (const marker of [
  'pub struct SourceArchiveInspector',
  'pub fn inspect(&self, archive_path: &Path)',
  'scan_safe_archive(&archive_path, Some(destination), limits)',
  'scan_safe_archive(archive_path, None, self.limits)',
  'buffer[..chunk].iter().any(|byte| *byte != 0)',
  'validate_archive_terminator',
  'pub struct CasArchivePublisher',
  'pub fn publish(',
  'copy_and_hash_archive(',
  'fs::hard_link(&temporary_path, &destination)',
  'validate_published_archive(&destination, expected_digest, limits, false)',
  'publisher_never_commits_under_a_mismatched_digest',
]) {
  assert.ok(reader.includes(marker), `strict archive reader is missing ${marker}`);
}

assert.ok(
  cliManifest.includes('rustok-build-source.workspace = true') &&
    cli.includes('SourceArchiveBuilder::new(limits)') &&
    cli.includes('SourceArchiveInspector::new(source_archive_limits()?)'),
  'module authoring CLI must reuse the shared writer and inspector',
);
for (const duplicate of ['fn ustar_header(', 'fn read_block(', 'tar::']) {
  assert.ok(!cli.includes(duplicate), `module authoring CLI duplicates archive logic: ${duplicate}`);
}

assert.ok(
  !manifest.includes('tar =') &&
    !reader.includes('#[allow(') &&
    !writer.includes('#[allow(') &&
    !reader.includes('todo!') &&
    !writer.includes('todo!') &&
    !reader.includes('unimplemented!') &&
    !writer.includes('unimplemented!'),
  'source archive boundary must remain explicit and free of suppressions or stubs',
);

console.log('[verify-module-source-archive] deterministic writer, atomic publisher, and shared strict reader verified');
