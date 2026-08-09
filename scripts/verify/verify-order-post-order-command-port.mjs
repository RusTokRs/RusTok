#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const read = (path) => readFileSync(path, 'utf8');
const portPath = 'crates/rustok-order/src/post_order_command.rs';
const libPath = 'crates/rustok-order/src/lib.rs';
const recordPath = 'crates/rustok-commerce/docs/order-post-order-command-owner-capability-2026-08-09.md';

const port = read(portPath);
const lib = read(libPath);
const record = read(recordPath);

const requireText = (text, marker, message) => {
  if (!text.includes(marker)) throw new Error(message);
};

for (const marker of [
  'pub trait OrderPostOrderCommandPort',
  'async fn create_change(',
  'async fn cancel_change(',
  'async fn create_return(',
  'async fn cancel_return(',
  'pub struct OrderPostOrderCommandRuntime',
  'context.require_policy(PortCallPolicy::write())',
  '.create_order_change(tenant_id, actor_id, request.order_id, request.input)',
  '.cancel_order_change(tenant_id, request.change_id, request.input)',
  '.create_return(tenant_id, request.order_id, request.input)',
  '.cancel_return(tenant_id, request.return_id, request.input)',
  'PortErrorKind::Unavailable',
  'PortErrorKind::InvariantViolation',
]) {
  requireText(port, marker, `${portPath}: missing ${marker}`);
}

for (const marker of [
  'mod post_order_command;',
  'OrderPostOrderCommandPort',
  'OrderPostOrderCommandRuntime',
  'in_process_order_post_order_command_port',
]) {
  requireText(lib, marker, `${libPath}: missing ${marker}`);
}

for (const forbidden of [
  'error.to_string()',
  'format!("{error}',
  'PortError::new(kind, code, error',
]) {
  if (port.includes(forbidden)) {
    throw new Error(`${portPath}: technical error text must stay out of public PortError: ${forbidden}`);
  }
}

requireText(
  record,
  'source_complete_unvalidated',
  `${recordPath}: source validation state must remain explicit`,
);
requireText(
  record,
  'does not claim durable replay receipts',
  `${recordPath}: replay limitation must remain explicit`,
);

console.log('order post-order command port source guard passed');
