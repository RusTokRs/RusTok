#!/usr/bin/env node

import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs';

const prefix = '[compare-index-storage-evidence]';

const preflightInputs = (args) => {
  const inputs = [];
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') return null;
    if (argument === '--input' && args[index + 1] && !args[index + 1].startsWith('--')) {
      inputs.push(args[++index]);
    } else if (argument === '--output' && args[index + 1] && !args[index + 1].startsWith('--')) {
      index += 1;
    } else {
      return null;
    }
  }
  return inputs;
};

const main = async () => {
  const inputs = preflightInputs(process.argv.slice(2));
  if (inputs !== null) {
    for (const input of inputs) validatePacketReadOrdering(input);
  }
  await import('./compare-index-storage-evidence-core.mjs');
};

try {
  await main();
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
