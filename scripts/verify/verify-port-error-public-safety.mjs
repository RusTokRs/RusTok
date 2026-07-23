#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const source = readFileSync(new URL('crates/rustok-api/src/ports.rs', root), 'utf8');
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText(
  'const PUBLIC_UNAVAILABLE_MESSAGE: &str = "the requested capability is temporarily unavailable";',
  'stable unavailable message',
);
requireText(
  'const PUBLIC_INVARIANT_MESSAGE: &str = "the requested operation could not be completed safely";',
  'stable invariant message',
);
requireText('fn sanitize_public_message(', 'central sanitizer');
requireText('impl Serialize for PortError', 'serialization sanitizer');
requireText("impl<'de> Deserialize<'de> for PortError", 'deserialization sanitizer');
requireText(
  'PortErrorKind::Unavailable => PUBLIC_UNAVAILABLE_MESSAGE.to_string()',
  'unavailable fail-closed mapping',
);
requireText(
  'PortErrorKind::InvariantViolation => PUBLIC_INVARIANT_MESSAGE.to_string()',
  'invariant fail-closed mapping',
);
requireText(
  'fn serde_boundaries_reapply_technical_message_sanitization()',
  'serde bypass regression test',
);
forbidText(
  '#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]\npub struct PortError',
  'derived PortError serde bypass',
);

if (failures.length > 0) {
  console.error('Public PortError safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ PortError constructors and serde boundaries sanitize technical public messages',
);
