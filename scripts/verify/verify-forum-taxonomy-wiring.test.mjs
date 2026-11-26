import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { spaced } from './lib/forum-source-verification.mjs';
import { inspectWiring, WIRING_RULES } from './verify-forum-taxonomy-wiring.mjs';

for (const rule of WIRING_RULES) {
  const [file, label, start, end, marker] = rule;
  const source = readFileSync(new URL(`../../${file}`, import.meta.url), 'utf8');
  test(`Taxonomy wiring accepts ${label}`, () => assert.deepEqual(inspectWiring(source, rule), []));
  test(`Taxonomy wiring rejects loss of ${label}`, () => {
    // Replace only this bounded edge, not a same-named call in another resolver.
    const from = source.indexOf(start);
    const to = end === null ? source.length : source.indexOf(end, from + start.length);
    assert.ok(from >= 0 && to > from);
    const body = spaced(source.slice(from, to));
    assert.ok(body.includes(spaced(marker)), `fixture must mutate ${label}`);
    const changed = source.slice(0, from) + body.replace(spaced(marker), 'missing_taxonomy_capability_edge') + '\n' + source.slice(to);
    assert.notDeepEqual(inspectWiring(changed, rule), []);
  });
}

test('Taxonomy wiring fails closed if the owner region is removed', () => {
  const rule = WIRING_RULES[0];
  assert.notDeepEqual(inspectWiring('// missing owner factory', rule), []);
});

// The existing route contract guard must follow live owner files, not removed include slices.
const { mkdtempSync, mkdirSync, rmSync, writeFileSync } = await import('node:fs');
const { tmpdir } = await import('node:os');
const { spawnSync } = await import('node:child_process');
const path = await import('node:path');
const { fileURLToPath } = await import('node:url');
const root = fileURLToPath(new URL('../../', import.meta.url));
const routeGuard = path.join(root, 'scripts/verify/verify-forum-category-route-storefront-transport.mjs');

for (const mutation of [null, 'factory', 'owner']) {
  test(`category transport guard uses live Taxonomy composition: ${mutation || 'current'}`, t => {
    const directory = mkdtempSync(path.join(tmpdir(), 'forum-taxonomy-route-'));
    t.after(() => rmSync(directory, { recursive: true, force: true }));
    const text = readFileSync(routeGuard, 'utf8');
    const files = new Set([...text.matchAll(/["'](crates\/[^"']+)["']/g)].map(match => match[1]));
    assert.ok(files.size > 10);
    for (const relative of files) {
      if (mutation === 'owner' && relative.endsWith('/services/category_route.rs')) continue;
      let source = readFileSync(path.join(root, relative), 'utf8');
      if (mutation === 'factory' && relative.endsWith('/graphql/category_route_query.rs')) {
        assert.ok(source.includes('.category_route_service(db.clone())'));
        source = source.replace('.category_route_service(db.clone())', '.missing_route_factory(db.clone())');
      }
      const destination = path.join(directory, relative);
      mkdirSync(path.dirname(destination), { recursive: true });
      writeFileSync(destination, source);
    }
    const result = spawnSync(process.execPath, [routeGuard], { encoding: 'utf8', cwd: root,
      env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: directory } });
    assert.ifError(result.error);
    assert.equal(result.status, mutation ? 1 : 0, result.stdout + result.stderr);
  });
}
