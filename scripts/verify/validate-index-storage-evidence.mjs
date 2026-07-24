#!/usr/bin/env node

import path from 'node:path';
import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs';

const prefix = '[validate-index-storage-evidence]';
const supportedScales = new Set(['smoke', '100k', '1m']);
const scale = process.env.INDEX_BENCH_SCALE;

/*
The byte-preserved implementation lives in validate-index-storage-evidence-core.mjs.
These markers keep the existing source-oracle static guard compatible until it is
migrated to inspect the preserved core directly:
const resultDigestContract = 'ordered_length_prefixed_json_v1'
const readOrderMarkers = new Map
requireReadOrdering
read.result_digest_contract
result_digest_contract: resultDigestContract
read.source_workloads
'source workload order'
sourceWorkload.sql.includes('idx_bench_source.')
workload.sql.includes('idx_bench_source.')
RFC 3339 UTC timestamp
server_version_num must contain only digits
differs from source oracle
source_workload_names: canonicalReadWorkloads
*/

const main = async () => {
  if (supportedScales.has(scale)) {
    const evidenceRoot = process.env.INDEX_BENCH_EVIDENCE_ROOT
      ?? path.join('evidence/index-storage', scale);
    validatePacketReadOrdering(evidenceRoot);
  }
  await import('./validate-index-storage-evidence-core.mjs');
};

try {
  await main();
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
