#!/usr/bin/env node
/**
 * Behavioural tests for the shared data-table host gate.
 *
 * The gate is a source analyser, so these tests feed it the real tree and then mutate one thing at a
 * time: a duplicated table wiring, a lost page size, a hand-rolled page count, a new raw `<Table>`,
 * a Rust dependency that drops `default-features = false`, and a `csr` feature that stops forwarding
 * the adapter. Each mutation must produce the failure the gate promises, and the untouched tree must
 * stay green — otherwise the gate is decorative.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  collectSources,
  evaluateHostRules,
  defaultRepoRoot,
  PAGINATED_TABLES,
  RAW_TABLE_RATCHET,
  SHELL,
  STATIC_TABLES,
} from "./verify-product-next-data-table-host.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
assert.equal(repoRoot, defaultRepoRoot, "the gate must resolve the repository it lives in");

const base = collectSources(repoRoot);

const clone = (sources) => new Map(sources);

function failuresAfter(mutate) {
  const { files, manifests } = { files: clone(base.files), manifests: clone(base.manifests) };
  mutate(files, manifests);
  return evaluateHostRules({ files, manifests });
}

const find = (failures, fragment) => failures.find((failure) => failure.includes(fragment));

test("the checked-in tree satisfies every table-host rule", () => {
  assert.deepEqual(evaluateHostRules(base), []);
});

test("the paginated wrappers are one-liners over the shell with their own configuration", () => {
  for (const { file, pageSize, debounceMs } of PAGINATED_TABLES) {
    const content = base.files.get(file);
    assert.ok(content, `${file} must exist`);
    assert.match(content, /DataTableShell/, `${file} must render through the shell`);
    assert.doesNotMatch(content, /useDataTable\(/, `${file} must not call the hook itself`);
    if (pageSize !== undefined) assert.match(content, new RegExp(`defaultPageSize=\\{${pageSize}\\}`));
    if (debounceMs !== undefined) assert.match(content, new RegExp(`debounceMs=\\{${debounceMs}\\}`));
  }
});

test("a duplicated table wiring is rejected", () => {
  const duplicate = "apps/next-admin/src/features/example/duplicate-table.tsx";
  const failures = failuresAfter((files) => {
    files.set(
      duplicate,
      [
        "'use client';",
        "import { parseAsInteger, useQueryState } from 'nuqs';",
        "import { useDataTable } from '@/shared/hooks/use-data-table';",
        "export function DuplicateTable() {",
        "  const [pageSize] = useQueryState('perPage', parseAsInteger.withDefault(20));",
        "  const { table } = useDataTable({ pageCount: 1, shallow: false });",
        "  return null;",
        "}",
        "",
      ].join("\n"),
    );
  });
  assert.ok(find(failures, duplicate), `expected a failure naming ${duplicate}`);
  assert.ok(find(failures, SHELL), "the failure must point at the single owner");
});

test("a wrapper that keeps its own page-count maths is rejected", () => {
  const victim = PAGINATED_TABLES[3].file;
  const failures = failuresAfter((files) => {
    files.set(
      victim,
      files.get(victim).replace(
        "    <DataTableShell",
        "    const pageCount = Math.ceil(totalItems / pageSize);\n    return (\n    <DataTableShell",
      ),
    );
  });
  assert.ok(find(failures, `${victim}: divides by a page size`), "unfloored page count must fail");
});

test("a wrapper that loses its page size or debounce is rejected", () => {
  const sized = PAGINATED_TABLES.find(({ pageSize }) => pageSize !== undefined).file;
  const debounced = PAGINATED_TABLES.find(({ debounceMs }) => debounceMs !== undefined).file;

  const lostSize = failuresAfter((files) => {
    files.set(
      sized,
      files.get(sized).replace(new RegExp(`\\s*defaultPageSize=\\{\\d+\\}`), ""),
    );
  });
  assert.ok(find(lostSize, `${sized}: lost its page size`));

  const lostDebounce = failuresAfter((files) => {
    files.set(
      debounced,
      files.get(debounced).replace(new RegExp(`\\s*debounceMs=\\{\\d+\\}`), ""),
    );
  });
  assert.ok(find(lostDebounce, `${debounced}: lost its debounce`));
});

test("a static table that goes back to its own markup is rejected", () => {
  const victim = STATIC_TABLES[0];
  const failures = failuresAfter((files) => {
    files.set(victim, `${files.get(victim)}\nfunction Legacy() {\n  return <Table><TableBody /></Table>;\n}\n`);
  });
  assert.ok(find(failures, `${victim}: still builds its own <Table> markup`));
});

test("the raw-markup ratchet rejects new copies and flags stale entries", () => {
  const offender = "apps/next-admin/src/features/example/legacy-table.tsx";
  const added = failuresAfter((files) => {
    files.set(offender, "export const Legacy = () => <Table><TableBody /></Table>;\n");
  });
  assert.ok(find(added, "new files build raw <Table> markup"));
  assert.ok(find(added, offender));

  const stale = failuresAfter((files) => {
    files.delete(RAW_TABLE_RATCHET[0]);
  });
  assert.ok(find(stale, "table-host ratchet is stale"));
  assert.ok(find(stale, RAW_TABLE_RATCHET[0]));
});

test("removing the shell is rejected instead of silently passing", () => {
  const failures = failuresAfter((files) => {
    files.delete(SHELL);
  });
  assert.ok(find(failures, SHELL), "the missing owner must be reported");
});

test("a Rust dependency that drops default-features = false is rejected", () => {
  const victim = "crates/modules/rustok-brand/admin/Cargo.toml";
  const failures = failuresAfter((_, manifests) => {
    manifests.set(
      victim,
      manifests.get(victim).replace(
        'rustok-grid-leptos = { workspace = true, default-features = false }',
        "rustok-grid-leptos.workspace = true",
      ),
    );
  });
  assert.ok(find(failures, victim), "a workspace-inherited grid dependency must fail");
  assert.ok(find(failures, "default-features = false"));
});

test("a feature that stops forwarding the adapter is rejected, the adapter itself is exempt", () => {
  const victim = "crates/modules/rustok-workflow/admin/Cargo.toml";
  const failures = failuresAfter((_, manifests) => {
    manifests.set(
      victim,
      manifests.get(victim).replace('"leptos/hydrate",\n  "leptos-auth/hydrate",\n  "rustok-grid-leptos/hydrate",', '"leptos/hydrate",\n  "leptos-auth/hydrate",'),
    );
  });
  assert.ok(find(failures, `${victim}: feature "hydrate" must forward`));

  const adapter = "crates/ui/rustok-grid/leptos/Cargo.toml";
  assert.ok(base.manifests.has(adapter), "the adapter manifest must be collected");
  assert.deepEqual(
    failuresAfter((_, manifests) => {
      manifests.set(adapter, `${manifests.get(adapter)}\n# untouched\n`);
    }),
    [],
    "the adapter keeps the ssr package default and must not be flagged",
  );
});

test("the workspace root keeps the neutral grid default", () => {
  const failures = failuresAfter((_, manifests) => {
    manifests.set(
      "Cargo.toml",
      manifests
        .get("Cargo.toml")
        .replace(
          'rustok-grid-leptos = { path = "crates/ui/rustok-grid/leptos", default-features = false }',
          'rustok-grid-leptos = { path = "crates/ui/rustok-grid/leptos" }',
        ),
    );
  });
  assert.ok(find(failures, "Cargo.toml: rustok-grid-leptos must set default-features = false"));
});
