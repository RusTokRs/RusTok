import { access, readFile } from 'node:fs/promises';

const paths = {
  flyLib: 'crates/fly/src/lib.rs',
  flyError: 'crates/fly/src/error.rs',
  snapshotFacade: 'crates/fly/src/snapshot.rs',
  snapshotModel: 'crates/fly/src/snapshot/model.rs',
  snapshotCatalog: 'crates/fly/src/snapshot/catalog.rs',
  snapshotDiff: 'crates/fly/src/snapshot/diff.rs',
  snapshotTests: 'crates/fly/src/snapshot/tests.rs',
  commandModel: 'crates/fly/src/command/model.rs',
  commandEditor: 'crates/fly/src/command/editor.rs',
  commandTests: 'crates/fly/src/command/tests.rs',
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
  'FlyError::SnapshotHashMismatch',
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
requireMarkers('commandModel', [
  'RestoreSnapshot',
  'pub fn restore_snapshot(snapshot: ProjectSnapshot)',
], 'snapshot command schema');
requireMarkers('commandEditor', [
  'pub fn restore_snapshot(',
  'self.apply(EditorCommand::restore_snapshot(snapshot.clone()))',
  'EditorCommand::RestoreSnapshot { snapshot }',
  '*document = snapshot.restore()?;',
], 'history-safe snapshot restore');
requireMarkers('commandTests', [
  'snapshot_restore_is_hash_verified_and_participates_in_history',
  'tampered_snapshot_does_not_change_document_or_history',
  'editor.undo().expect("undo restore")',
  'editor.redo().expect("redo restore")',
], 'snapshot transaction regression coverage');

try {
  await access('crates/rustok-page-builder/admin/src/editor/snapshot_panel.rs');
  failures.push(
    'orphan hydrated snapshot_panel.rs must not exist; restore UI must call the history-safe snapshot command',
  );
} catch {
  // Expected: a future SSR panel must be built on FlyEditor::restore_snapshot.
}

if (failures.length > 0) {
  console.error('Fly snapshot verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly snapshots verified.');