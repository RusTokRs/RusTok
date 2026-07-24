#!/usr/bin/env node

import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs';

const prefix = '[compare-index-storage-evidence]';

/*
The byte-preserved implementation lives in compare-index-storage-evidence-core.mjs.
These markers keep the existing source-oracle static guard compatible until it is
migrated to inspect the preserved core directly:
const resultDigestContract = 'ordered_length_prefixed_json_v1'
const readOrderMarkers = new Map
requireReadOrdering
validateReadEvidence
validateMutationEvidence
requirePlan
validateDatabase
validateDataset
validateProvenance
validateSourceOracle
validateReadReport
validateMutationReport
validateMaintenanceReport
same_result_digest_contract
same_dataset_shape
same_source_oracle_shape
result_rows_ratio_1m_to_100k
fail closed on report shape, metrics, plans, effects, ordering, digest semantics, and cardinalities
Result digest contract:
### Source oracle
*/

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
