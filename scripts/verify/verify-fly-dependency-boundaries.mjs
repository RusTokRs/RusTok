#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import process from 'node:process';

const root = resolve(import.meta.dirname, '../..');

const rules = [
  {
    crate: 'fly',
    manifest: 'crates/fly/Cargo.toml',
    required: [],
    forbidden: ['leptos', 'dioxus', 'rustok-']
  },
  {
    crate: 'fly-ui',
    manifest: 'crates/fly-ui/Cargo.toml',
    required: ['fly = { path = "../fly" }'],
    forbidden: ['leptos', 'dioxus', 'rustok-']
  },
  {
    crate: 'fly-leptos',
    manifest: 'crates/fly-leptos/Cargo.toml',
    required: [
      'fly = { path = "../fly" }',
      'fly-ui = { path = "../fly-ui" }',
      'leptos.workspace = true'
    ],
    forbidden: ['dioxus', 'rustok-']
  }
];

const errors = [];

for (const rule of rules) {
  const manifestPath = resolve(root, rule.manifest);
  let manifest;
  try {
    manifest = await readFile(manifestPath, 'utf8');
  } catch (error) {
    errors.push(`${rule.crate}: cannot read ${rule.manifest}: ${error.message}`);
    continue;
  }

  const normalized = manifest.toLowerCase();
  for (const dependency of rule.required) {
    if (!manifest.includes(dependency)) {
      errors.push(`${rule.crate}: missing required dependency declaration: ${dependency}`);
    }
  }
  for (const forbidden of rule.forbidden) {
    if (normalized.includes(forbidden)) {
      errors.push(`${rule.crate}: forbidden dependency marker found: ${forbidden}`);
    }
  }
}

if (errors.length > 0) {
  console.error('Fly dependency boundary verification failed:');
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exitCode = 1;
} else {
  console.log('Fly dependency boundaries are valid.');
}
