#!/usr/bin/env node
// ORM declaration evidence only. Generated SeaORM schema execution is covered by
// tests/audience_relation_schema.rs and still requires Cargo.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const RELATION_SCOPES = [
  'category_audience', 'category_topic_create_audience', 'category_reply_create_audience',
  'category_moderation_audience', 'topic_audience', 'topic_reply_create_audience',
];
export const RELATION_RULES = RELATION_SCOPES.flatMap(scope => ['role', 'channel', 'group', 'user'].map(suffix => ({
  scope, suffix, file: `crates/modules/rustok-forum/src/entities/forum_${scope}_${suffix}.rs`,
})));

export function relationMapping(source) {
  const relation = source.slice(source.indexOf('pub enum Relation'));
  const attribute = name => relation.match(new RegExp(`\\b${name}\\s*=\\s*"([^"]+)"`))?.[1];
  const columns = name => (attribute(name)?.match(/(?:\w+::)*Column::(\w+)/g) || [])
    .map(column => column.split('::').at(-1));
  return { parent: attribute('belongs_to'), from: columns('from'), to: columns('to'),
    onDelete: attribute('on_delete'), onUpdate: attribute('on_update') };
}
export function inspectRelation(source, rule) {
  const actual = relationMapping(source);
  const key = rule.scope.startsWith('category') ? 'CategoryId' : 'TopicId';
  const pair = ['TenantId', key];
  const errors = [];
  if (actual.parent !== `super::forum_${rule.scope}_policy::Entity`) errors.push('policy owner');
  if (JSON.stringify(actual.from) !== JSON.stringify(pair)) errors.push('tenant-scoped source columns');
  if (JSON.stringify(actual.to) !== JSON.stringify(pair)) errors.push('tenant-scoped policy columns');
  if (actual.onDelete !== 'Cascade' || actual.onUpdate !== 'Cascade') errors.push('cascade semantics');
  return errors.map(error => `${rule.file}: missing ${error}`);
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = process.env.RUSTOK_VERIFY_REPO_ROOT || fileURLToPath(new URL('../../', import.meta.url));
  const failures = RELATION_RULES.flatMap(rule => {
    try { return inspectRelation(readFileSync(path.join(root, rule.file), 'utf8'), rule); }
    catch { return [`${rule.file}: required relation source is missing`]; }
  });
  if (failures.length) { console.error(failures.join('\n')); process.exitCode = 1; }
  else console.log(`Forum audience policy relations preserve ${RELATION_RULES.length} tenant-scoped composite FKs (source only).`);
}
