import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { collectDbMultilingualContractFailures } from "./verify-db-multilingual-contract.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const contractPath = "docs/architecture/database-multilingual-contract.json";
const mcpMigration = "crates/modules/rustok-mcp/src/migrations/m20260319_000001_create_mcp_management_tables.rs";

// Overlay only the registry: all other inputs remain the actual owner contracts.
// Negative cases must still fail, rather than making a moved migration optional.
function withContract(change, assertion) {
  const contract = JSON.parse(readFileSync(path.join(repoRoot, contractPath), "utf8"));
  change(contract);
  assertion(collectDbMultilingualContractFailures(repoRoot, JSON.stringify(contract)));
}

function mcpGap(contract) {
  return contract.known_gaps.find((gap) => gap.id === "mcp-management-display-copy");
}

test("multilingual registry follows the MCP migration owner without closing the display-copy gap", () => {
  withContract((contract) => {
    const gap = mcpGap(contract);
    assert.equal(gap.owner, "rustok-mcp");
    assert.equal(gap.status, "documented_open");
    assert.equal(gap.path, mcpMigration);
  }, (failures) => assert.deepEqual(failures, []));
});

test("multilingual guard rejects the obsolete foundation MCP migration path", () => {
  const obsolete = "crates/utils/rustok-migrations/src/m20260319_000001_create_mcp_management_tables.rs";
  withContract((contract) => { mcpGap(contract).path = obsolete; }, (failures) => {
    assert.ok(failures.includes(`${obsolete}: required file is missing`), failures.join("\n"));
  });
});

test("multilingual guard continues to require evidence from the moved migration", () => {
  withContract((contract) => {
    mcpGap(contract).evidence_markers.push("nonexistent_mcp_localized_copy_marker");
  }, (failures) => {
    assert.ok(failures.includes(`${mcpMigration}: missing marker "nonexistent_mcp_localized_copy_marker"`), failures.join("\n"));
  });
});

test("multilingual guard continues to require the known gap in its owning documentation", () => {
  withContract((contract) => { mcpGap(contract).id = "unrecorded-mcp-copy-gap"; }, (failures) => {
    assert.ok(failures.some((failure) => failure.includes("missing known gap unrecorded-mcp-copy-gap")), failures.join("\n"));
  });
});
