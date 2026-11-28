/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

// TypeScript 7 removed the legacy JavaScript compiler API. Use Next's bundled
// SWC transform for this isolated TSX test fixture instead of depending on it.
const { transformSync } = require('next/dist/build/swc');

module.exports = function transpile(source) {
  if (typeof transformSync !== 'function') {
    throw new Error('Next SWC transformSync is unavailable to the Fly React test loader');
  }
  return transformSync(source, {
    filename: this?.resourcePath ?? 'fly-react-harness.tsx',
    jsc: {
      target: 'es2022',
      parser: {
        syntax: 'typescript',
        tsx: true,
      },
      transform: {
        react: {
          runtime: 'automatic',
        },
      },
    },
    module: {
      type: 'es6',
    },
  }).code;
};
