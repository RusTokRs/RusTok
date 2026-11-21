#!/usr/bin/env node

/**
 * RusToK Universal Design Tokens Generator
 * Reads UI/tokens/tokens.json and generates type-safe constants for:
 * 1. Flutter / Dart (UI/tokens/rustok_tokens.g.dart & app_ui_kit)
 * 2. Rust (crates/ui/rustok-ui/src/tokens_generated.rs)
 * 3. TypeScript (packages/rustok-ui-controls/src/tokens.ts)
 */

import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const rootDir = resolve(__dirname, '../..');

const tokensPath = resolve(rootDir, 'UI/tokens/tokens.json');
const dartOutputs = [
  resolve(rootDir, 'UI/tokens/rustok_tokens.g.dart'),
  resolve(rootDir, 'rustok_mobile/packages/app_ui_kit/lib/tokens/rustok_tokens.g.dart')
];
const rustOutput = resolve(rootDir, 'crates/ui/rustok-ui/src/tokens_generated.rs');
const tsOutput = resolve(rootDir, 'packages/rustok-ui-controls/src/tokens.ts');

function hexToDartColor(hex) {
  const cleanHex = hex.replace('#', '');
  return `Color(0xFF${cleanHex.toUpperCase()})`;
}

function toScreamingSnake(str) {
  return str.replace(/([a-z])([A-Z])/g, '$1_$2').toUpperCase();
}

function generateDart(tokens) {
  const { radius, spacing, colors } = tokens;

  return `// GENERATED CODE - DO NOT MODIFY BY HAND
// Generated from UI/tokens/tokens.json
// Part of RusToK Design System (FFA Compatible)

import 'package:flutter/painting.dart';

/// Design tokens for RusToK Design System.
abstract final class RusTokRadius {
  RusTokRadius._();

${Object.entries(radius)
  .map(([key, val]) => `  static const double ${key} = ${val}.0;`)
  .join('\n')}

${Object.entries(radius)
  .map(
    ([key, val]) =>
      `  static const BorderRadius border${key.charAt(0).toUpperCase() + key.slice(1)} = BorderRadius.all(Radius.circular(${val}.0));`
  )
  .join('\n')}
}

/// Spacing tokens.
abstract final class RusTokSpacing {
  RusTokSpacing._();

${Object.entries(spacing)
  .map(([key, val]) => `  static const double space${key} = ${val}.0;`)
  .join('\n')}
}

/// Color tokens for light and dark themes.
abstract final class RusTokColors {
  RusTokColors._();

  // Light Theme
${Object.entries(colors.light)
  .map(([key, val]) => `  static const Color light${key.charAt(0).toUpperCase() + key.slice(1)} = ${hexToDartColor(val)};`)
  .join('\n')}

  // Dark Theme
${Object.entries(colors.dark)
  .map(([key, val]) => `  static const Color dark${key.charAt(0).toUpperCase() + key.slice(1)} = ${hexToDartColor(val)};`)
  .join('\n')}
}
`;
}

function generateRust(tokens) {
  const { radius, spacing, colors } = tokens;

  return `// GENERATED CODE - DO NOT MODIFY BY HAND
// Generated from UI/tokens/tokens.json
// Part of RusToK Design System (FFA Compatible)

//! Generated numeric and semantic constants matching UI tokens.

#![allow(missing_docs)]

pub mod radius_px {
${Object.entries(radius)
  .map(([key, val]) => `    pub const ${key.toUpperCase()}: f64 = ${val}.0;`)
  .join('\n')}
}

pub mod spacing_px {
${Object.entries(spacing)
  .map(([key, val]) => `    pub const SPACE_${key}: f64 = ${val}.0;`)
  .join('\n')}
}

pub mod colors_hex {
    pub mod light {
${Object.entries(colors.light)
  .map(([key, val]) => `        pub const ${toScreamingSnake(key)}: &str = "${val}";`)
  .join('\n')}
    }

    pub mod dark {
${Object.entries(colors.dark)
  .map(([key, val]) => `        pub const ${toScreamingSnake(key)}: &str = "${val}";`)
  .join('\n')}
    }
}
`;
}

function generateTypeScript(tokens) {
  const { radius, spacing, colors } = tokens;

  return `// GENERATED CODE - DO NOT MODIFY BY HAND
// Generated from UI/tokens/tokens.json
// Part of RusToK Design System (FFA Compatible)

export const RUSTOK_RADIUS_PX = {
${Object.entries(radius)
  .map(([key, val]) => `  ${key}: ${val},`)
  .join('\n')}
} as const;

export const RUSTOK_SPACING_PX = {
${Object.entries(spacing)
  .map(([key, val]) => `  '${key}': ${val},`)
  .join('\n')}
} as const;

export const RUSTOK_COLORS = {
  light: {
${Object.entries(colors.light)
  .map(([key, val]) => `    ${key}: '${val}',`)
  .join('\n')}
  },
  dark: {
${Object.entries(colors.dark)
  .map(([key, val]) => `    ${key}: '${val}',`)
  .join('\n')}
  },
} as const;

export type RusTokRadiusKey = keyof typeof RUSTOK_RADIUS_PX;
export type RusTokSpacingKey = keyof typeof RUSTOK_SPACING_PX;
export type RusTokTheme = keyof typeof RUSTOK_COLORS;
`;
}

function main() {
  console.log(`Reading tokens from: ${tokensPath}`);
  const tokens = JSON.parse(readFileSync(tokensPath, 'utf8'));

  // 1. Dart
  const dartContent = generateDart(tokens);
  for (const outPath of dartOutputs) {
    if (existsSync(dirname(outPath))) {
      writeFileSync(outPath, dartContent, 'utf8');
      console.log(`[SUCCESS] Generated Dart tokens: ${outPath}`);
    }
  }

  // 2. Rust
  const rustContent = generateRust(tokens);
  mkdirSync(dirname(rustOutput), { recursive: true });
  writeFileSync(rustOutput, rustContent, 'utf8');
  console.log(`[SUCCESS] Generated Rust tokens: ${rustOutput}`);

  // 3. TypeScript
  const tsContent = generateTypeScript(tokens);
  mkdirSync(dirname(tsOutput), { recursive: true });
  writeFileSync(tsOutput, tsContent, 'utf8');
  console.log(`[SUCCESS] Generated TypeScript tokens: ${tsOutput}`);
}

main();
