import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { RELATION_RULES, inspectRelation, relationMapping } from './verify-forum-audience-relation-schema.mjs';

const sources = RELATION_RULES.map(rule => readFileSync(new URL(`../../${rule.file}`, import.meta.url), 'utf8'));
for (const [index, rule] of RELATION_RULES.entries()) {
  const source = sources[index];
  test(`audience relation accepts composite FK: ${rule.scope}/${rule.suffix}`, () => {
    assert.deepEqual(inspectRelation(source, rule), []);
  });
  for (const attribute of ['from', 'to']) {
    test(`audience relation rejects missing tenant ${attribute}: ${rule.scope}/${rule.suffix}`, () => {
      const changed = source.replace(new RegExp(`(\\b${attribute}\\s*=\\s*")([^"\\n]+)(")`), (_all, left, value, right) =>
        left + value.replace(/\(?[^,]*Column::TenantId,\s*/, '').replace(/\)$/, '') + right);
      assert.notEqual(changed, source, 'mutation must remove the actual tenant column');
      assert.notDeepEqual(inspectRelation(changed, rule), []);
    });
  }
}

test('SQLite checks declaration-projected FK rejection and cascade for all 24 policy relations', () => {
  // This intentionally executes only a projection of the declared FK tuples,
  // not SeaORM-generated DDL, domain services or the production migrations.
  const mappings = sources.map(relationMapping).map((mapping, index) => ({ ...mapping, ...RELATION_RULES[index] }));
  const python = String.raw`
import json, sqlite3, sys
snake = {'TenantId': 'tenant_id', 'CategoryId': 'category_id', 'TopicId': 'topic_id'}
for m in json.load(sys.stdin):
    c = sqlite3.connect(':memory:')
    c.execute('PRAGMA foreign_keys=ON')
    key = 'category_id' if m['scope'].startswith('category') else 'topic_id'
    c.execute(f'CREATE TABLE policy (tenant_id BLOB, {key} BLOB, PRIMARY KEY(tenant_id, {key}))')
    fk_from = ', '.join(snake[col] for col in m['from'])
    fk_to = ', '.join(snake[col] for col in m['to'])
    c.execute(f'CREATE TABLE relation (tenant_id BLOB, {key} BLOB, member BLOB, PRIMARY KEY(tenant_id, {key}, member), FOREIGN KEY({fk_from}) REFERENCES policy({fk_to}) ON DELETE CASCADE ON UPDATE CASCADE)')
    # Two tenant-local policies sharing a subject show that deleting one tenant
    # cannot remove the other's relation. Domain ownership is tested in Rust.
    for tenant in [b'one', b'two']:
        c.execute('INSERT INTO policy VALUES (?, ?)', (tenant, b'subject'))
        c.execute('INSERT INTO relation VALUES (?, ?, ?)', (tenant, b'subject', b'member'))
    try:
        c.execute('INSERT INTO relation VALUES (?, ?, ?)', (b'foreign', b'subject', b'member'))
    except sqlite3.IntegrityError as e:
        assert 'FOREIGN KEY constraint failed' in str(e), (m, str(e))
    else:
        raise AssertionError(('foreign tenant was admitted', m))
    c.execute('DELETE FROM policy WHERE tenant_id=?', (b'one',))
    assert c.execute('SELECT tenant_id FROM relation').fetchall() == [(b'two',)], m
    c.execute('UPDATE policy SET tenant_id=? WHERE tenant_id=?', (b'new-two', b'two'))
    assert c.execute('SELECT tenant_id FROM relation').fetchall() == [(b'new-two',)], m
    c.close()
print('24 declaration-projected FK/cascade scenarios passed; not SeaORM/service runtime evidence')
`;
  const result = spawnSync('python3', ['-c', python], { input: JSON.stringify(mappings), encoding: 'utf8' });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

test('SQLite reproduces the previous single-column to composite-parent schema error', () => {
  const result = spawnSync('python3', ['-c', String.raw`
import sqlite3
c = sqlite3.connect(':memory:')
c.execute('PRAGMA foreign_keys=ON')
c.execute('CREATE TABLE policy (tenant BLOB, subject BLOB, PRIMARY KEY(tenant,subject))')
c.execute('CREATE TABLE relation (tenant BLOB, subject BLOB REFERENCES policy(subject))')
try:
    c.execute("INSERT INTO relation VALUES (X'01', X'02')")
except sqlite3.OperationalError as e:
    assert 'foreign key mismatch' in str(e), str(e)
else:
    raise AssertionError('previous mapping must fail under enforced SQLite foreign keys')
`], { encoding: 'utf8' });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
});
