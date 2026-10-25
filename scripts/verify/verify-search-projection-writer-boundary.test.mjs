#!/usr/bin/env node

/**
 * Fixture test for `verify-search-projection-writer-boundary.mjs`.
 *
 * The guard is only useful if it fails for the right reason, so each case builds a
 * minimal `rustok-search/src` tree and asserts the exact diagnostic. Run with
 * `node scripts/verify/verify-search-projection-writer-boundary.test.mjs`.
 */

import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const verifier = path.resolve("scripts/verify/verify-search-projection-writer-boundary.mjs");
const srcDir = "crates/modules/rustok-search/src";

const DRIVER = `
    pub(crate) struct SearchProjectionDriver;
    impl SearchProjectionDriver {
        pub(crate) async fn rebuild_tenant(&self, tenant_id: Uuid) -> Result<()> { Ok(()) }
        pub(crate) async fn refresh_entity(&self, tenant_id: Uuid, entity_type: &str) -> Result<()> {
            if self.source.refresh_scope(entity_type) == SearchProjectionRefreshScope::Tenant {
                return Ok(());
            }
            Ok(())
        }
        pub(crate) async fn delete_entity(&self, tenant_id: Uuid, entity_type: &str) -> Result<()> { Ok(()) }
        pub(crate) async fn delete_tenant(&self, tenant_id: Uuid) -> Result<()> { Ok(()) }
        async fn delete_scope(&self) -> Result<()> {
            let sql = "DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = $2";
            let values = vec![tenant_id.into(), self.source.source_module().into()];
            Ok(())
        }
        fn validate_document(&self, document: &SearchProjectionDocument) -> Result<()> {
            if document.document_id.is_nil() {
                return Err(Error::Validation("nil".to_string()));
            }
            Ok(())
        }
    }
    fn validate_entity_type(entity_type: &str) -> Result<()> { Ok(()) }
`;

const INGESTION = `
    fn build_projection_driver() {}
    fn owner_drivers() {}
    fn refresh_owner_entity() {}
    fn rebuild_owner_scope() {}
    fn delete_owner_entity() {}
    fn toggle_owner_scope() {}
`;

const PRIVATE_WRITER = `
    async fn write() {
        let sql = "INSERT INTO search_documents (document_key) VALUES ($1)";
    }
`;

function fixture({
  driver = DRIVER,
  ingestion = INGESTION,
  files = {},
  omit = [],
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-search-writer-boundary-"));

  const defaults = {
    "projection_driver.rs": driver,
    "ingestion.rs": ingestion,
    "blog_projector.rs": PRIVATE_WRITER,
    "projector_core.rs": PRIVATE_WRITER,
  };

  for (const [name, content] of Object.entries({ ...defaults, ...files })) {
    if (omit.includes(name)) continue;
    const target = path.join(root, srcDir, name);
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, content);
  }

  return root;
}

function run(root) {
  const result = spawnSync(process.execPath, [verifier], {
    encoding: "utf8",
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
  });
  return { status: result.status, stderr: result.stderr, stdout: result.stdout };
}

function withFixture(options, assertion) {
  const root = fixture(options);
  try {
    assertion(run(root));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("accepts the migrated tree with the two documented private writers", () => {
  withFixture({}, ({ status, stdout, stderr }) => {
    assert.equal(status, 0, stderr);
    assert.match(stdout, /one neutral projection writer/);
  });
});

test("rejects a new private writer", () => {
  withFixture(
    { files: { "product_legacy_writer.rs": PRIVATE_WRITER } },
    ({ status, stderr }) => {
      assert.equal(status, 1);
      assert.match(stderr, /product_legacy_writer\.rs writes search_documents outside/);
    },
  );
});

test("rejects a retired projector that comes back", () => {
  withFixture(
    { files: { "pages_projector.rs": "pub(crate) struct PagesSearchProjector;" } },
    ({ status, stderr }) => {
      assert.equal(status, 1);
      assert.match(stderr, /pages_projector\.rs was replaced by projection_driver\.rs/);
    },
  );
});

test("rejects an allowlist entry that no longer writes", () => {
  withFixture(
    { files: { "blog_projector.rs": "// nothing writes here any more" } },
    ({ status, stderr }) => {
      assert.equal(status, 1);
      assert.match(stderr, /blog_projector\.rs no longer writes search_documents/);
    },
  );
});

test("rejects an allowlist entry whose file was deleted", () => {
  withFixture({ omit: ["projector_core.rs"] }, ({ status, stderr }) => {
    assert.equal(status, 1);
    assert.match(stderr, /projector_core\.rs is gone, so its remainingPrivateWriters entry/);
  });
});

test("rejects a driver that names an owner module", () => {
  withFixture(
    { driver: `${DRIVER}\n    const MODULE: &str = "forum";\n` },
    ({ status, stderr }) => {
      assert.equal(status, 1);
      assert.match(stderr, /projection_driver\.rs names a module/);
    },
  );
});

test("rejects a driver that lost the entity delete capability", () => {
  const driver = DRIVER.replace(
    "pub(crate) async fn delete_entity(&self, tenant_id: Uuid, entity_type: &str) -> Result<()> { Ok(()) }\n",
    "",
  );
  withFixture({ driver }, ({ status, stderr }) => {
    assert.equal(status, 1);
    assert.match(stderr, /missing the storage contract marker: pub\(crate\) async fn delete_entity\(/);
  });
});

test("rejects a driver that stopped scoping deletes by source module", () => {
  const driver = DRIVER.replace(
    '"DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = $2"',
    '"DELETE FROM search_documents WHERE tenant_id = $1"',
  );
  withFixture({ driver }, ({ status, stderr }) => {
    assert.equal(status, 1);
    assert.match(stderr, /DELETE FROM search_documents WHERE tenant_id = \$1 AND source_module = \$2/);
  });
});

test("rejects ingestion that goes back to a per-module projector", () => {
  withFixture(
    { ingestion: `${INGESTION}\n    struct PagesSearchProjector;\n` },
    ({ status, stderr }) => {
      assert.equal(status, 1);
      assert.match(stderr, /still references PagesSearchProjector/);
    },
  );
});
