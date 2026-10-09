#!/usr/bin/env node

/**
 * RusToK Design Tokens -> Flutter/Dart Generator
 * Reads UI/tokens/tokens.json and generates type-safe Dart classes for Flutter.
 *
 * Usage:
 *   node scripts/generate/generate-flutter-tokens.mjs [--output path/to/tokens.g.dart]
 */

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const rootDir = resolve(__dirname, '../..');

const tokensPath = resolve(rootDir, 'UI/tokens/tokens.json');
const defaultOutputPath = resolve(rootDir, 'UI/tokens/rustok_tokens.g.dart');

function hexToDartColor(hex) {
  const cleanHex = hex.replace('#', '');
  return `Color(0xFF${cleanHex.toUpperCase()})`;
}

function generateDartCode(tokens) {
  const { radius, spacing, colors } = tokens;

  let dart = `// GENERATED CODE - DO NOT MODIFY BY HAND
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
  .map(([key, val]) => `  static const BorderRadius border${key.charAt(0).toUpperCase() + key.slice(1)} = BorderRadius.all(Radius.circular(${val}.0));`)
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

  // Light Palette
${Object.entries(colors.light)
  .map(([key, val]) => `  static const Color light${key.charAt(0).toUpperCase() + key.slice(1)} = ${hexToDartColor(val)};`)
  .join('\n')}

  // Dark Palette
${Object.entries(colors.dark)
  .map(([key, val]) => `  static const Color dark${key.charAt(0).toUpperCase() + key.slice(1)} = ${hexToDartColor(val)};`)
  .join('\n')}
}
`;
  return dart;
}

try {
  const raw = readFileSync(tokensPath, 'utf-8');
  const tokens = JSON.parse(raw);
  const dartCode = generateDartCode(tokens);

  const outputPath = process.argv[2] === '--output' && process.argv[3]
    ? resolve(process.cwd(), process.argv[3])
    : defaultOutputPath;

  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, dartCode, 'utf-8');

  console.log(`[RusToK Codegen] Successfully generated Flutter tokens -> ${outputPath}`);
} catch (err) {
  console.error('[RusToK Codegen] Error generating Flutter tokens:', err);
  process.exit(1);
}
