import { readFile } from 'node:fs/promises';

const paths = {
  commandFacade: 'crates/fly/src/command.rs',
  commandPatch: 'crates/fly/src/command/patch.rs',
  commandModel: 'crates/fly/src/command/model.rs',
  commandEditor: 'crates/fly/src/command/editor.rs',
  commandTests: 'crates/fly/src/command/tests.rs',
  snapshotModel: 'crates/fly/src/snapshot/model.rs',
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

requireMarkers('commandFacade', [
  'mod editor;',
  'mod model;',
  'mod patch;',
  'pub use editor::*;',
  'pub use model::*;',
  'pub use patch::*;',
  '#[cfg(test)]\nmod tests;',
], 'command facade');
for (const forbidden of [
  'pub struct ComponentPatch',
  'pub enum EditorCommand',
  'pub struct FlyEditor',
  'fn apply_asset_command(',
]) {
  rejectMarker(
    'commandFacade',
    forbidden,
    `command facade must not own implementation marker ${forbidden}`,
  );
}
requireMarkers('commandPatch', [
  'pub struct ComponentPatch',
  'pub fn set_component_type',
  'pub fn clear_component_type',
  'pub fn set_tag_name',
  'pub fn set_provider',
  'pub fn set_schema_version',
  'pub fn set_field',
  'pub fn remove_field',
  'pub fn set_attribute',
  'pub fn remove_attribute',
  'pub fn merge_style',
  'pub fn replace_style',
  'pub fn clear_style',
  'COMPONENT_TYPE_FIELD => component.component_type = None',
  'typed_reserved_fields_set_and_clear_without_extension_leaks',
], 'typed component patch contract');
requireMarkers('commandModel', [
  'pub enum EditorCommand',
  'RestoreSnapshot',
  'pub fn restore_snapshot(snapshot: ProjectSnapshot)',
  'pub struct History',
  'pub struct ProjectHash',
  'pub struct RevisionState',
], 'command/history/revision model');
requireMarkers('commandEditor', [
  'pub struct FlyEditor',
  'pub fn restore_snapshot(',
  'self.apply(EditorCommand::restore_snapshot(snapshot.clone()))',
  'EditorCommand::RestoreSnapshot { snapshot }',
  '*document = snapshot.restore()?;',
  'self.history.push(HistoryEntry',
  'self.revision.mark_changed(&self.document)',
], 'transactional editor engine');
requireMarkers('snapshotModel', [
  'pub fn restore(&self)',
  'FlyError::SnapshotHashMismatch',
], 'hash-verified snapshot restore');
requireMarkers('commandTests', [
  'style_patch_merges_and_can_remove_individual_properties',
  'batch_is_atomic_and_creates_one_history_entry',
  'failed_batch_does_not_change_document_or_history',
  'snapshot_restore_is_hash_verified_and_participates_in_history',
  'tampered_snapshot_does_not_change_document_or_history',
], 'command transaction regression coverage');

if (failures.length > 0) {
  console.error('Fly command transaction verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly command transactions verified.');