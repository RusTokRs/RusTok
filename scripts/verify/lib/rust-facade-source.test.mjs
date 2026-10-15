import assert from 'node:assert/strict';
import test from 'node:test';

import { readComposedRustSource } from './rust-facade-source.mjs';

const files = {
  'src/root.rs': [
    'mod base;',
    '#[path = "parts/second.rs"]',
    'mod second;',
    'include!("legacy.rs");',
  ].join('\n'),
  'src/legacy.rs': 'LEGACY',
  'src/parts/second.rs': '#[path = "../third.rs"]\nmod third;',
  'src/third.rs': 'THIRD',
};

const read = (file) => {
  if (!(file in files)) throw new Error(`unexpected read: ${file}`);
  return files[file];
};

test('composes the entry file with every declared part', () => {
  const composed = readComposedRustSource(read, 'src/root.rs');
  // Depth-first in declaration order: the entry file, then each declared part in full before
  // the next declaration. `mod base;` is a plain module declaration, not a file directive.
  assert.equal(
    composed,
    [
      files['src/root.rs'],
      files['src/parts/second.rs'],
      files['src/third.rs'],
      files['src/legacy.rs'],
    ].join('\n'),
  );
});

test('resolves declarations relative to the declaring file across directories', () => {
  const composed = readComposedRustSource(read, 'src/parts/second.rs');
  assert.equal(composed, [files['src/parts/second.rs'], files['src/third.rs']].join('\n'));
});

test('reads a shared part once even when declared twice', () => {
  const diamond = {
    'a.rs': '#[path = "b.rs"]\nmod b;\n#[path = "c.rs"]\nmod c;',
    'b.rs': '#[path = "d.rs"]\nmod d;',
    'c.rs': '#[path = "d.rs"]\nmod d;',
    'd.rs': 'D',
  };
  const composed = readComposedRustSource((file) => diamond[file], 'a.rs');
  assert.equal(composed.split('D').length - 1, 1);
});

test('is stable across repeated calls in one process', () => {
  assert.equal(readComposedRustSource(read, 'src/root.rs'), readComposedRustSource(read, 'src/root.rs'));
});

test('fails loud when a declaration points at a file that no longer exists', () => {
  const broken = { 'a.rs': '#[path = "gone.rs"]\nmod gone;' };
  assert.throws(() => readComposedRustSource((file) => {
    if (!(file in broken)) throw new Error(`ENOENT: ${file}`);
    return broken[file];
  }, 'a.rs'), /ENOENT: gone.rs/);
});
