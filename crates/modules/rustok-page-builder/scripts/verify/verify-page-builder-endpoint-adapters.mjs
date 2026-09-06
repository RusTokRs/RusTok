#!/usr/bin/env node
import { readFileSync } from 'node:fs';

const files = {
  lib: 'crates/rustok-page-builder/src/lib.rs',
  adapters: 'crates/rustok-page-builder/src/adapters.rs',
  docs: 'crates/rustok-page-builder/docs/README.md',
  plan: 'crates/rustok-page-builder/docs/implementation-plan.md',
};

const read = (path) => readFileSync(path, 'utf8');
const fail = (message) => {
  console.error(`FAIL ${message}`);
  process.exitCode = 1;
};
const requireContains = (body, needle, label) => {
  if (!body.includes(needle)) fail(`${label}: missing ${needle}`);
};

const lib = read(files.lib);
const adapters = read(files.adapters);
const docs = read(files.docs);
const plan = read(files.plan);

requireContains(lib, 'pub mod adapters;', files.lib);

for (const marker of [
  'PageBuilderGraphqlEndpointInput',
  'PageBuilderLeptosServerFunctionInput',
  'PageBuilderEndpointSuccess',
  'PageBuilderEndpointError',
  'PageBuilderEndpointResult',
  'handle_page_builder_graphql_endpoint',
  'handle_page_builder_leptos_server_function_endpoint',
  'dispatch_graphql_envelope(handlers, context, auth, input.request)',
  'dispatch_leptos_server_function_envelope(handlers, context, auth, input.request)',
  'PageBuilderCapabilityRequest',
  'PageBuilderTransportSuccess',
  'PageBuilderTransportError',
]) {
  requireContains(adapters, marker, files.adapters);
}

requireContains(docs, 'src/adapters.rs', files.docs);
requireContains(docs, 'handle_page_builder_graphql_endpoint', files.docs);
requireContains(docs, 'handle_page_builder_leptos_server_function_endpoint', files.docs);
requireContains(plan, 'endpoint adapter seam', files.plan);
requireContains(plan, 'verify-page-builder-endpoint-adapters.mjs', files.plan);

if (!process.exitCode) console.log('PASS page-builder endpoint adapter markers are in sync');
