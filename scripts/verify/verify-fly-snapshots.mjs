import { access, readFile } from 'node:fs/promises';

const paths = {
  flyLib: 'crates/fly/src/lib.rs',
  flyError: 'crates/fly/src/error.rs',
  snapshotFacade: 'crates/fly/src/snapshot.rs',
  snapshotModel: 'crates/fly/src/snapshot/model.rs',
  snapshotCatalog: 'crates/fly/src/snapshot/catalog.rs',
  snapshotDiff: 'crates/fly/src/snapshot/diff.rs',
  snapshotTests: 'crates/fly/src/snapshot/tests.rs',
};

const source = Object.fromEntries(await Promise.all(
  Object.entries(paths).map(async ([key, path]) => [key, await readFile(path, 'utf8')]),
));
const failures = [];
const requireMarker = (key, marker, message) => {
  if (!source[key].includes(marker)) failures.push(message);
};
const rejectMarker = (key, marker, message) => {
  if (source[key].includes(marker)) failures.push(message);
};
const requireMarkers = (key, markers, label) => {
  for (const marker of markers) requireMarker(key, marker, `${label} is missing ${marker}`);
};

requireMarkers('flyLib', [
  'mod snapshot;',
  'pub use snapshot::*;',
  'pub use component_visit::{visit_project_components, ComponentVisit};',
], 'Fly snapshot module wiring');
requireMarkers('flyError', [
  'SnapshotNotFound(String)',
  'SnapshotHashMismatch',
], 'snapshot error contract');
requireMarkers('snapshotFacade', [
  'mod catalog;',
  'mod diff;',
  'mod model;',
  'pub use catalog::*;',
  'pub use diff::*;',
  'pub use model::*;',
  '#[cfg(test)]\nmod tests;',
], 'snapshot facade');
requireMarkers('snapshotModel', [
  'pub struct ProjectSnapshot',
  'pub fn restore(&self)',
  'pub struct ProjectDiffSummary',
  'pub fn change_count(&self)',
], 'snapshot models');
requireMarkers('snapshotCatalog', [
  'pub struct SnapshotCatalog',
  'pub fn capture(',
  'pub fn compare_with_current(',
  'FlyError::SnapshotNotFound',
  'super::diff::compare_projects',
], 'bounded snapshot catalog');
requireMarkers('snapshotDiff', [
  'pub fn compare_projects(',
  'visit_project_components(&document.project',
  'visit.path()',
  'AssetCatalog::from_document',
  'StyleRuleCatalog::from_document',
], 'structural snapshot diff');
for (const forbidden of [
  '.project.visit_components(',
  'root.visit(',
]) {
  rejectMarker(
    'snapshotDiff',
    forbidden,
    `snapshot diff must use the canonical read-only visitor instead of ${forbidden}`,
  );
}
requireMarkers('snapshotTests', [
  'snapshots_restore_and_verify_hash',
  'catalog_evicts_old_snapshots',
  'structural_diff_tracks_component_changes',
  'catalog_compares_snapshot_with_current',
  'anonymous_components_use_canonical_paths_in_diffs',
  'missing_snapshot_is_explicit',
], 'snapshot regression coverage');

try {
  await access('crates/rustok-page-builder/admin/src/editor/snapshot_panel.rs');
  failures.push(
    'orphan hydrated snapshot_panel.rs must not exist; restore UI requires a command/history-safe SSR design',
  );
} catch {
  // Expected: the old panel was not registered and bypassed editor history/revision semantics.
}

if (failures.length > 0) {
  console.error('Fly snapshot verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly snapshots verified.');