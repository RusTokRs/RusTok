import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const shellSource = await readFile(
  new URL('../../../src/widgets/data-table/data-table-shell.tsx', import.meta.url),
  'utf8'
);
const hookSource = await readFile(
  new URL('../../../src/shared/hooks/use-data-table.ts', import.meta.url),
  'utf8'
);

test('server data table forwards its resolved page size into useDataTable defaults', () => {
  assert.match(shellSource, /defaultPageSize = 20/);
  assert.match(
    shellSource,
    /initialState:\s*\{\s*pagination:\s*\{\s*pageIndex:\s*0,\s*pageSize\s*\}\s*\}/s
  );
  assert.match(hookSource, /initialState\?\.pagination\?\.pageSize \?\? 10/);
});

test('text filter values restored from the URL remain scalar strings', () => {
  assert.match(hookSource, /filters\.push\(\{\s*id: key,\s*value\s*\}\)/s);
  assert.doesNotMatch(hookSource, /value\.split\(/);
});
