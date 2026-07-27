import { createHash } from 'node:crypto';

export const MANIFEST_CONTRACT = 'index_partition_evidence_manifest_v1';
export const PACKET_CONTRACT = 'index_partition_evidence_packet_v1';
export const ADMISSION_CONTRACT = 'index_partition_admission_v1';
export const SHADOW_PLAN_VERSION = 'tenant_hash_shadow_v1';
export const PLAN_DIGEST_CONTRACT = 'normalized_partition_plan_v1';
export const TENANT_COVERAGE_BPS = 10_000;

const RAW_ARTIFACT_ROLES = [
  'baseline',
  'shadow',
  'query',
  'mutation',
  'maintenance',
  'cutover',
];

const isObject = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);

const requireObject = (value, label) => {
  if (!isObject(value)) throw new Error(`${label} must be an object`);
  return value;
};

const requireArray = (value, label) => {
  if (!Array.isArray(value)) throw new Error(`${label} must be an array`);
  return value;
};

const requireNonEmptyString = (value, label) => {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
};

const requireInteger = (value, label, minimum = 0) => {
  if (!Number.isSafeInteger(value) || value < minimum) {
    throw new Error(`${label} must be an integer >= ${minimum}`);
  }
  return value;
};

const requireNumber = (value, label, minimum = 0) => {
  if (!Number.isFinite(value) || value < minimum) {
    throw new Error(`${label} must be a finite number >= ${minimum}`);
  }
  return value;
};

const requireBoolean = (value, label) => {
  if (typeof value !== 'boolean') throw new Error(`${label} must be a boolean`);
  return value;
};

const requireDigest = (value, label) => {
  if (typeof value !== 'string' || !/^[0-9a-f]{64}$/u.test(value)) {
    throw new Error(`${label} must be a lowercase SHA-256 digest`);
  }
  return value;
};

const requireTimestamp = (value, label) => {
  requireNonEmptyString(value, label);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/u.test(value)
      || !Number.isFinite(Date.parse(value))) {
    throw new Error(`${label} must be an RFC 3339 UTC timestamp`);
  }
  return value;
};

const canonicalValue = (value) => {
  if (Array.isArray(value)) return value.map(canonicalValue);
  if (!isObject(value)) return value;
  return Object.fromEntries(
    Object.keys(value).sort().map((key) => [key, canonicalValue(value[key])]),
  );
};

export const canonicalJson = (value) => JSON.stringify(canonicalValue(value));
export const sha256Hex = (value) => createHash('sha256').update(value).digest('hex');

const validateModulus = (modulus) => {
  requireInteger(modulus, 'manifest.modulus', 2);
  if (modulus > 128 || (modulus & (modulus - 1)) !== 0) {
    throw new Error('manifest.modulus must be a power of two between 2 and 128');
  }
};

const requireThresholds = (thresholds) => {
  requireObject(thresholds, 'manifest.thresholds');
  requireInteger(thresholds.minimum_total_rows, 'manifest.thresholds.minimum_total_rows', 1);
  requireInteger(thresholds.minimum_total_bytes, 'manifest.thresholds.minimum_total_bytes', 1);
  requireInteger(
    thresholds.minimum_distinct_tenants,
    'manifest.thresholds.minimum_distinct_tenants',
    2,
  );
  requireInteger(
    thresholds.maximum_query_p95_regression_bps,
    'manifest.thresholds.maximum_query_p95_regression_bps',
  );
  requireInteger(
    thresholds.maximum_mutation_p95_regression_bps,
    'manifest.thresholds.maximum_mutation_p95_regression_bps',
  );
  requireInteger(
    thresholds.maximum_wal_amplification_bps,
    'manifest.thresholds.maximum_wal_amplification_bps',
    TENANT_COVERAGE_BPS,
  );
  requireInteger(
    thresholds.maximum_partition_size_to_mean_bps,
    'manifest.thresholds.maximum_partition_size_to_mean_bps',
    TENANT_COVERAGE_BPS,
  );
  requireInteger(
    thresholds.maximum_cutover_lock_ms,
    'manifest.thresholds.maximum_cutover_lock_ms',
    1,
  );
};

const requireRepetitions = (repetitions) => {
  requireObject(repetitions, 'manifest.repetitions');
  for (const key of ['query', 'mutation', 'maintenance', 'cutover']) {
    requireInteger(repetitions[key], `manifest.repetitions.${key}`, 1);
  }
};

const requireLocales = (locales) => {
  requireArray(locales, 'manifest.locales');
  if (locales.length === 0) throw new Error('manifest.locales must not be empty');
  if (new Set(locales).size !== locales.length) {
    throw new Error('manifest.locales must not contain duplicates');
  }
  for (const [index, locale] of locales.entries()) {
    requireNonEmptyString(locale, `manifest.locales[${index}]`);
  }
};

export const validateManifestInput = (manifest) => {
  requireObject(manifest, 'manifest');
  if (manifest.contract !== MANIFEST_CONTRACT) {
    throw new Error(`manifest.contract must be ${MANIFEST_CONTRACT}`);
  }
  if (manifest.repository !== 'RusTokRs/RusTok') {
    throw new Error('manifest.repository must be RusTokRs/RusTok');
  }
  if (typeof manifest.commit !== 'string' || !/^[0-9a-f]{40}$/u.test(manifest.commit)) {
    throw new Error('manifest.commit must be a full lowercase Git commit SHA');
  }
  if (typeof manifest.run_key !== 'string'
      || !/^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/u.test(manifest.run_key)) {
    throw new Error('manifest.run_key must be a bounded stable run identifier');
  }
  if (manifest.postgres_image !== 'postgres:16') {
    throw new Error('manifest.postgres_image must be postgres:16');
  }
  if (manifest.strategy !== 'tenant_hash') {
    throw new Error('manifest.strategy must be tenant_hash');
  }
  if (manifest.plan_digest_contract !== PLAN_DIGEST_CONTRACT) {
    throw new Error(`manifest.plan_digest_contract must be ${PLAN_DIGEST_CONTRACT}`);
  }
  validateModulus(manifest.modulus);
  requireLocales(manifest.locales);
  requireRepetitions(manifest.repetitions);
  requireThresholds(manifest.thresholds);
  return manifest;
};

export const computeEvidenceId = (manifestInput) => {
  validateManifestInput(manifestInput);
  return sha256Hex(canonicalJson(manifestInput));
};

export const deriveShadowRelations = (evidenceId, modulus) => {
  requireDigest(evidenceId, 'evidence_id');
  validateModulus(modulus);
  const definition = [
    'rustok-index-partition',
    SHADOW_PLAN_VERSION,
    evidenceId,
    'tenant_hash',
    String(modulus),
  ].join('\u001f');
  const definitionHash = sha256Hex(definition);
  const suffix = definitionHash.slice(0, 24);
  const relation = (source) => {
    const parent = `${source}_shadow_${suffix}`;
    const partitions = Array.from(
      { length: modulus },
      (_, remainder) => `${parent}_p${String(remainder).padStart(3, '0')}`,
    );
    return { source, parent, partitions };
  };
  return {
    definition_hash: definitionHash,
    entities: relation('index_entities'),
    links: relation('index_links'),
  };
};

const quoteIdentifier = (value) => `"${value.replaceAll('"', '""')}"`;

export const renderShadowBootstrapSql = (preparedManifest) => {
  validatePreparedManifest(preparedManifest);
  const lines = [
    '-- Generated by prepare-index-partition-evidence.mjs.',
    '-- Shadow bootstrap only: no production ALTER, DROP, RENAME, copy, or cutover.',
    'BEGIN;',
  ];
  for (const relation of [
    preparedManifest.shadow_relations.entities,
    preparedManifest.shadow_relations.links,
  ]) {
    lines.push(
      `CREATE TABLE ${quoteIdentifier(relation.parent)} (LIKE ${quoteIdentifier(relation.source)} INCLUDING DEFAULTS INCLUDING GENERATED INCLUDING IDENTITY INCLUDING STORAGE INCLUDING COMMENTS) PARTITION BY HASH (tenant_id);`,
      `COMMENT ON TABLE ${quoteIdentifier(relation.parent)} IS 'rustok-index-partition:${preparedManifest.evidence_id}';`,
    );
    relation.partitions.forEach((partition, remainder) => {
      lines.push(
        `CREATE TABLE ${quoteIdentifier(partition)} PARTITION OF ${quoteIdentifier(relation.parent)} FOR VALUES WITH (MODULUS ${preparedManifest.modulus}, REMAINDER ${remainder});`,
      );
    });
  }
  lines.push('COMMIT;', '');
  return lines.join('\n');
};

export const prepareManifest = (manifestInput) => {
  const input = structuredClone(validateManifestInput(manifestInput));
  const evidenceId = computeEvidenceId(input);
  return {
    ...input,
    evidence_id: evidenceId,
    shadow_plan_version: SHADOW_PLAN_VERSION,
    shadow_relations: deriveShadowRelations(evidenceId, input.modulus),
  };
};

export const validatePreparedManifest = (manifest) => {
  requireObject(manifest, 'packet.manifest');
  const {
    evidence_id: evidenceId,
    shadow_plan_version: shadowPlanVersion,
    shadow_relations: shadowRelations,
    ...input
  } = manifest;
  validateManifestInput(input);
  requireDigest(evidenceId, 'packet.manifest.evidence_id');
  if (computeEvidenceId(input) !== evidenceId) {
    throw new Error('packet.manifest.evidence_id does not match canonical manifest bytes');
  }
  if (shadowPlanVersion !== SHADOW_PLAN_VERSION) {
    throw new Error(`packet.manifest.shadow_plan_version must be ${SHADOW_PLAN_VERSION}`);
  }
  const expected = deriveShadowRelations(evidenceId, input.modulus);
  if (canonicalJson(shadowRelations) !== canonicalJson(expected)) {
    throw new Error('packet.manifest.shadow_relations do not match the deterministic plan');
  }
  return manifest;
};

const validateRelationEvidence = (relation, label) => {
  requireObject(relation, label);
  requireInteger(relation.rows, `${label}.rows`);
  requireInteger(relation.bytes, `${label}.bytes`, 1);
  requireDigest(relation.digest, `${label}.digest`);
};

const regressionBps = (baseline, shadow, label) => {
  requireNumber(baseline, `${label}.baseline`, Number.EPSILON);
  requireNumber(shadow, `${label}.shadow`);
  return Math.max(0, Math.round(((shadow - baseline) / baseline) * TENANT_COVERAGE_BPS));
};

const amplificationBps = (baseline, shadow, label) => {
  requireInteger(baseline, `${label}.baseline`, 1);
  requireInteger(shadow, `${label}.shadow`);
  return Math.round((shadow / baseline) * TENANT_COVERAGE_BPS);
};

const maximumSkewBps = (partitionBytes, modulus, label) => {
  requireArray(partitionBytes, label);
  if (partitionBytes.length !== modulus) {
    throw new Error(`${label} must contain exactly ${modulus} partition sizes`);
  }
  partitionBytes.forEach((value, index) => requireInteger(value, `${label}[${index}]`, 1));
  const mean = partitionBytes.reduce((sum, value) => sum + value, 0) / modulus;
  return Math.round((Math.max(...partitionBytes) / mean) * TENANT_COVERAGE_BPS);
};

const validateNamedRuns = (runs, label, expectedCount) => {
  requireArray(runs, label);
  if (runs.length !== expectedCount) {
    throw new Error(`${label} must contain exactly ${expectedCount} runs`);
  }
  const names = runs.map((run, index) => {
    requireObject(run, `${label}[${index}]`);
    return requireNonEmptyString(run.name, `${label}[${index}].name`);
  });
  if (new Set(names).size !== names.length) throw new Error(`${label} contains duplicate names`);
};

const pushThresholdReason = (reasons, condition, code, actual, maximum) => {
  if (condition) reasons.push({ code, actual, maximum });
};

const validateRunProvenance = (provenance, manifest) => {
  requireObject(provenance, 'packet.run_provenance');
  for (const field of ['repository', 'commit', 'run_key', 'job', 'runner_os', 'runner_arch']) {
    requireNonEmptyString(provenance[field], `packet.run_provenance.${field}`);
  }
  if (provenance.repository !== manifest.repository) {
    throw new Error('packet.run_provenance.repository must match the manifest');
  }
  if (provenance.commit !== manifest.commit) {
    throw new Error('packet.run_provenance.commit must match the manifest');
  }
  if (provenance.run_key !== manifest.run_key) {
    throw new Error('packet.run_provenance.run_key must match the manifest');
  }
};

const validateRawArtifacts = (artifacts) => {
  requireObject(artifacts, 'packet.raw_artifacts');
  const keys = Object.keys(artifacts).sort();
  if (canonicalJson(keys) !== canonicalJson([...RAW_ARTIFACT_ROLES].sort())) {
    throw new Error(`packet.raw_artifacts must contain exactly: ${RAW_ARTIFACT_ROLES.join(', ')}`);
  }
  for (const role of RAW_ARTIFACT_ROLES) {
    requireDigest(artifacts[role], `packet.raw_artifacts.${role}`);
  }
};

export const validatePartitionPacket = (packet) => {
  requireObject(packet, 'packet');
  if (packet.contract !== PACKET_CONTRACT) {
    throw new Error(`packet.contract must be ${PACKET_CONTRACT}`);
  }
  validatePreparedManifest(packet.manifest);
  const completedAt = Date.parse(requireTimestamp(packet.completed_at, 'packet.completed_at'));
  validateRunProvenance(packet.run_provenance, packet.manifest);
  validateRawArtifacts(packet.raw_artifacts);

  const database = requireObject(packet.database, 'packet.database');
  requireNonEmptyString(database.version, 'packet.database.version');
  if (typeof database.server_version_num !== 'string'
      || !/^16\d{4}$/u.test(database.server_version_num)) {
    throw new Error('packet.database.server_version_num must describe PostgreSQL 16');
  }
  if (database.jit !== 'off') throw new Error('packet.database.jit must be off');
  requireNonEmptyString(database.system_identifier, 'packet.database.system_identifier');
  if (!/^\d+$/u.test(database.system_identifier)) {
    throw new Error('packet.database.system_identifier must contain only digits');
  }
  requireNonEmptyString(database.database_name, 'packet.database.database_name');

  const baseline = requireObject(packet.baseline, 'packet.baseline');
  const baselineAt = Date.parse(requireTimestamp(baseline.generated_at, 'packet.baseline.generated_at'));
  requireInteger(baseline.distinct_tenants, 'packet.baseline.distinct_tenants', 1);
  const audit = requireObject(baseline.tenant_predicate_audit, 'packet.baseline.tenant_predicate_audit');
  requireInteger(audit.total_templates, 'packet.baseline.tenant_predicate_audit.total_templates', 1);
  requireInteger(audit.tenant_scoped_templates, 'packet.baseline.tenant_predicate_audit.tenant_scoped_templates');
  if (audit.tenant_scoped_templates > audit.total_templates) {
    throw new Error('tenant-scoped template count cannot exceed total templates');
  }
  validateRelationEvidence(baseline.entities, 'packet.baseline.entities');
  validateRelationEvidence(baseline.links, 'packet.baseline.links');

  const shadow = requireObject(packet.shadow, 'packet.shadow');
  const shadowAt = Date.parse(requireTimestamp(shadow.generated_at, 'packet.shadow.generated_at'));
  if (baselineAt > shadowAt || shadowAt > completedAt) {
    throw new Error('packet timestamps must satisfy baseline <= shadow <= completed');
  }
  requireBoolean(shadow.caught_up, 'packet.shadow.caught_up');
  requireBoolean(shadow.foreign_keys_validated, 'packet.shadow.foreign_keys_validated');
  requireInteger(shadow.orphan_links, 'packet.shadow.orphan_links');
  validateRelationEvidence(shadow.entities, 'packet.shadow.entities');
  validateRelationEvidence(shadow.links, 'packet.shadow.links');

  const repetitions = packet.manifest.repetitions;
  validateNamedRuns(packet.query_runs, 'packet.query_runs', repetitions.query);
  validateNamedRuns(packet.mutation_runs, 'packet.mutation_runs', repetitions.mutation);
  validateNamedRuns(packet.maintenance_runs, 'packet.maintenance_runs', repetitions.maintenance);
  requireArray(packet.cutover_rehearsals, 'packet.cutover_rehearsals');
  if (packet.cutover_rehearsals.length !== repetitions.cutover) {
    throw new Error(`packet.cutover_rehearsals must contain exactly ${repetitions.cutover} runs`);
  }

  let maximumQueryRegressionBps = 0;
  let queryPlanRegressions = 0;
  for (const [index, run] of packet.query_runs.entries()) {
    maximumQueryRegressionBps = Math.max(
      maximumQueryRegressionBps,
      regressionBps(run.baseline_p95_ms, run.shadow_p95_ms, `packet.query_runs[${index}]`),
    );
    requireDigest(run.baseline_plan_digest, `packet.query_runs[${index}].baseline_plan_digest`);
    requireDigest(run.shadow_plan_digest, `packet.query_runs[${index}].shadow_plan_digest`);
    if (run.baseline_plan_digest !== run.shadow_plan_digest) queryPlanRegressions += 1;
  }

  let maximumMutationRegressionBps = 0;
  let maximumWalAmplificationBps = 0;
  for (const [index, run] of packet.mutation_runs.entries()) {
    maximumMutationRegressionBps = Math.max(
      maximumMutationRegressionBps,
      regressionBps(run.baseline_p95_ms, run.shadow_p95_ms, `packet.mutation_runs[${index}]`),
    );
    maximumWalAmplificationBps = Math.max(
      maximumWalAmplificationBps,
      amplificationBps(
        run.baseline_wal_bytes,
        run.shadow_wal_bytes,
        `packet.mutation_runs[${index}].wal`,
      ),
    );
  }

  for (const [index, run] of packet.maintenance_runs.entries()) {
    requireNumber(run.baseline_vacuum_ms, `packet.maintenance_runs[${index}].baseline_vacuum_ms`);
    requireNumber(run.shadow_vacuum_ms, `packet.maintenance_runs[${index}].shadow_vacuum_ms`);
    requireInteger(run.baseline_dead_tuples, `packet.maintenance_runs[${index}].baseline_dead_tuples`);
    requireInteger(run.shadow_dead_tuples, `packet.maintenance_runs[${index}].shadow_dead_tuples`);
  }

  let maximumCutoverLockMs = 0;
  for (const [index, run] of packet.cutover_rehearsals.entries()) {
    requireObject(run, `packet.cutover_rehearsals[${index}]`);
    maximumCutoverLockMs = Math.max(
      maximumCutoverLockMs,
      requireInteger(run.lock_ms, `packet.cutover_rehearsals[${index}].lock_ms`),
    );
    requireBoolean(run.rollback_verified, `packet.cutover_rehearsals[${index}].rollback_verified`);
    requireBoolean(
      run.production_relations_unchanged,
      `packet.cutover_rehearsals[${index}].production_relations_unchanged`,
    );
  }

  const entitySkewBps = maximumSkewBps(
    shadow.entities.partition_bytes,
    packet.manifest.modulus,
    'packet.shadow.entities.partition_bytes',
  );
  const linkSkewBps = maximumSkewBps(
    shadow.links.partition_bytes,
    packet.manifest.modulus,
    'packet.shadow.links.partition_bytes',
  );
  const maximumPartitionSizeToMeanBps = Math.max(entitySkewBps, linkSkewBps);
  const tenantPredicateCoverageBps = Math.floor(
    (audit.tenant_scoped_templates * TENANT_COVERAGE_BPS) / audit.total_templates,
  );
  const totalRows = baseline.entities.rows + baseline.links.rows;
  const totalBytes = baseline.entities.bytes + baseline.links.bytes;
  const thresholds = packet.manifest.thresholds;
  const reasons = [];

  if (totalRows < thresholds.minimum_total_rows) {
    reasons.push({ code: 'below_minimum_rows', actual: totalRows, minimum: thresholds.minimum_total_rows });
  }
  if (totalBytes < thresholds.minimum_total_bytes) {
    reasons.push({ code: 'below_minimum_bytes', actual: totalBytes, minimum: thresholds.minimum_total_bytes });
  }
  if (baseline.distinct_tenants < thresholds.minimum_distinct_tenants) {
    reasons.push({
      code: 'insufficient_distinct_tenants',
      actual: baseline.distinct_tenants,
      minimum: thresholds.minimum_distinct_tenants,
    });
  }
  if (baseline.distinct_tenants < packet.manifest.modulus) {
    reasons.push({
      code: 'insufficient_tenants_for_modulus',
      actual: baseline.distinct_tenants,
      modulus: packet.manifest.modulus,
    });
  }
  if (tenantPredicateCoverageBps !== TENANT_COVERAGE_BPS) {
    reasons.push({
      code: 'tenant_predicate_coverage',
      actual_bps: tenantPredicateCoverageBps,
      required_bps: TENANT_COVERAGE_BPS,
    });
  }
  if (baseline.entities.rows !== shadow.entities.rows
      || baseline.entities.digest !== shadow.entities.digest) {
    reasons.push({ code: 'entity_digest_mismatch' });
  }
  if (baseline.links.rows !== shadow.links.rows || baseline.links.digest !== shadow.links.digest) {
    reasons.push({ code: 'link_digest_mismatch' });
  }
  if (!shadow.caught_up) reasons.push({ code: 'shadow_not_caught_up' });
  if (!shadow.foreign_keys_validated) reasons.push({ code: 'foreign_keys_not_validated' });
  if (shadow.orphan_links !== 0) reasons.push({ code: 'orphan_links', count: shadow.orphan_links });
  if (queryPlanRegressions !== 0) {
    reasons.push({ code: 'query_plan_regressions', count: queryPlanRegressions });
  }
  pushThresholdReason(
    reasons,
    maximumQueryRegressionBps > thresholds.maximum_query_p95_regression_bps,
    'query_latency_regression',
    maximumQueryRegressionBps,
    thresholds.maximum_query_p95_regression_bps,
  );
  pushThresholdReason(
    reasons,
    maximumMutationRegressionBps > thresholds.maximum_mutation_p95_regression_bps,
    'mutation_latency_regression',
    maximumMutationRegressionBps,
    thresholds.maximum_mutation_p95_regression_bps,
  );
  pushThresholdReason(
    reasons,
    maximumWalAmplificationBps > thresholds.maximum_wal_amplification_bps,
    'wal_amplification',
    maximumWalAmplificationBps,
    thresholds.maximum_wal_amplification_bps,
  );
  pushThresholdReason(
    reasons,
    maximumPartitionSizeToMeanBps > thresholds.maximum_partition_size_to_mean_bps,
    'partition_size_skew',
    maximumPartitionSizeToMeanBps,
    thresholds.maximum_partition_size_to_mean_bps,
  );
  pushThresholdReason(
    reasons,
    maximumCutoverLockMs > thresholds.maximum_cutover_lock_ms,
    'cutover_lock_exceeded',
    maximumCutoverLockMs,
    thresholds.maximum_cutover_lock_ms,
  );
  if (packet.cutover_rehearsals.some((run) => !run.rollback_verified)) {
    reasons.push({ code: 'rollback_not_verified' });
  }
  if (packet.cutover_rehearsals.some((run) => !run.production_relations_unchanged)) {
    reasons.push({ code: 'production_relations_changed' });
  }

  return {
    contract: ADMISSION_CONTRACT,
    evidence_id: packet.manifest.evidence_id,
    packet_digest: sha256Hex(canonicalJson(packet)),
    completed_at: packet.completed_at,
    run_provenance: structuredClone(packet.run_provenance),
    measurements: {
      total_rows: totalRows,
      total_bytes: totalBytes,
      distinct_tenants: baseline.distinct_tenants,
      tenant_predicate_coverage_bps: tenantPredicateCoverageBps,
      query_runs: packet.query_runs.length,
      mutation_runs: packet.mutation_runs.length,
      maintenance_runs: packet.maintenance_runs.length,
      cutover_rehearsals: packet.cutover_rehearsals.length,
      query_plan_regressions: queryPlanRegressions,
      maximum_query_p95_regression_bps: maximumQueryRegressionBps,
      maximum_mutation_p95_regression_bps: maximumMutationRegressionBps,
      maximum_wal_amplification_bps: maximumWalAmplificationBps,
      maximum_partition_size_to_mean_bps: maximumPartitionSizeToMeanBps,
      maximum_cutover_lock_ms: maximumCutoverLockMs,
      entity_digest_matches:
        baseline.entities.rows === shadow.entities.rows
        && baseline.entities.digest === shadow.entities.digest,
      link_digest_matches:
        baseline.links.rows === shadow.links.rows
        && baseline.links.digest === shadow.links.digest,
      shadow_caught_up: shadow.caught_up,
      foreign_keys_validated: shadow.foreign_keys_validated,
      orphan_links: shadow.orphan_links,
    },
    outcome: reasons.length === 0 ? 'admitted' : 'keep_unpartitioned',
    reasons,
  };
};
