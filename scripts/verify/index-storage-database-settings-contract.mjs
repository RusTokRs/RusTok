export const comparableDatabaseFields = Object.freeze([
  'server_version_num',
  'shared_buffers',
  'effective_cache_size',
  'work_mem',
  'random_page_cost',
  'jit',
  'standard_conforming_strings',
  'timezone',
  'date_style',
  'extra_float_digits',
]);

export const databaseSettingsSource =
  'read-report.json database metadata observed from the active PostgreSQL benchmark session after exact equality was verified against mutation-report.json and maintenance-report.json active-session metadata';

export const comparisonMethodologyKeys = Object.freeze([
  'source_oracle',
  'result_digest',
  'evidence_validation',
  'first_run',
  'warm_run',
  'automatic_winner_selection',
  'comparable_database_fields',
  'database_settings_source',
]);

const sameJson = (left, right) => JSON.stringify(left) === JSON.stringify(right);

export const requireComparisonDatabaseSettingsMethodology = (comparison, fail) => {
  const methodology = comparison?.methodology;
  if (!methodology || typeof methodology !== 'object' || Array.isArray(methodology)) {
    fail('comparison.methodology must be an object');
  }
  const actualKeys = Object.keys(methodology).sort();
  const expectedKeys = [...comparisonMethodologyKeys].sort();
  if (!sameJson(actualKeys, expectedKeys)) {
    fail('comparison methodology must contain exactly the canonical methodology fields');
  }
  if (!sameJson(methodology.comparable_database_fields, comparableDatabaseFields)) {
    fail(
      'comparison methodology comparable_database_fields must exactly match the canonical PostgreSQL database-settings contract',
    );
  }
  if (methodology.database_settings_source !== databaseSettingsSource) {
    fail(
      'comparison methodology database_settings_source must identify read metadata observed from the active PostgreSQL benchmark session after exact equality with mutation and maintenance active-session metadata',
    );
  }
  return methodology;
};
