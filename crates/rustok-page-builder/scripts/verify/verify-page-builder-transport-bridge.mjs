#!/usr/bin/env node
import { readFileSync } from 'node:fs';

const files = {
  lib: 'crates/rustok-page-builder/src/lib.rs',
  transport: 'crates/rustok-page-builder/src/transport.rs',
  service: 'crates/rustok-page-builder/src/service.rs',
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
const transport = read(files.transport);
const service = read(files.service);
const docs = read(files.docs);
const plan = read(files.plan);

requireContains(lib, 'pub mod transport;', files.lib);
requireContains(service, 'pub async fn handle(', files.service);
requireContains(service, 'PageBuilderCapabilityRequest::Preview', files.service);
requireContains(service, 'PageBuilderCapabilityRequest::Tree', files.service);
requireContains(service, 'PageBuilderCapabilityRequest::Properties', files.service);
requireContains(service, 'PageBuilderCapabilityRequest::Publish', files.service);

for (const marker of [
  'PageBuilderTransportKind',
  'Graphql',
  'LeptosServerFunction',
  'FutureMobileBridge',
  'PageBuilderTransportSuccess',
  'PageBuilderTransportError',
  'dispatch_transport_envelope',
  'dispatch_graphql_envelope',
  'dispatch_leptos_server_function_envelope',
  'AuthorizedPageBuilderHandlers',
  'handlers.handle(context, auth, request)',
  'error.kind()',
  'error.stable_code()',
]) {
  requireContains(transport, marker, files.transport);
}

requireContains(docs, 'transport bridge', files.docs);
requireContains(docs, 'dispatch_graphql_envelope', files.docs);
requireContains(docs, 'dispatch_leptos_server_function_envelope', files.docs);
requireContains(plan, 'transport bridge slice', files.plan);
requireContains(plan, 'verify-page-builder-transport-bridge.mjs', files.plan);

if (!process.exitCode) console.log('PASS page-builder transport bridge markers are in sync');
