#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { generateTypeDeclarations } from '../dist/typegen.js';
import { pseudoLocalizeFtl } from '../dist/index.js';

const args = process.argv.slice(2);
const command = args[0];

if (command !== 'typegen' && command !== 'pseudo') {
  console.log(`Usage:
  next-fluent typegen --input <path-to-ftl> --output <path-to-dts>
  next-fluent pseudo  --input <path-to-ftl> --output <path-to-ftl>`);
  process.exit(1);
}

let inputPath = '';
let outputPath = '';

for (let i = 1; i < args.length; i++) {
  if (args[i] === '--input' || args[i] === '-i') {
    inputPath = args[++i];
  } else if (args[i] === '--output' || args[i] === '-o') {
    outputPath = args[++i];
  }
}

if (!inputPath || !outputPath) {
  console.error('Error: Both --input and --output are required.');
  process.exit(1);
}

const resolvedInput = path.resolve(process.cwd(), inputPath);
const resolvedOutput = path.resolve(process.cwd(), outputPath);

if (!fs.existsSync(resolvedInput)) {
  console.error(`Error: Input file not found: ${resolvedInput}`);
  process.exit(1);
}

const ftlContent = fs.readFileSync(resolvedInput, 'utf8');

if (command === 'typegen') {
  const dtsContent = generateTypeDeclarations(ftlContent);
  fs.mkdirSync(path.dirname(resolvedOutput), { recursive: true });
  fs.writeFileSync(resolvedOutput, dtsContent, 'utf8');
  console.log(`[next-fluent] Successfully generated types at: ${outputPath}`);
} else if (command === 'pseudo') {
  const pseudoContent = pseudoLocalizeFtl(ftlContent);
  fs.mkdirSync(path.dirname(resolvedOutput), { recursive: true });
  fs.writeFileSync(resolvedOutput, pseudoContent, 'utf8');
  console.log(`[next-fluent] Successfully generated pseudo-locale at: ${outputPath}`);
}
