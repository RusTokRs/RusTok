#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function requireContains(text, needle, message) {
  if (!text.includes(needle)) {
    throw new Error(message);
  }
}

function requireAbsent(text, needle, message) {
  if (text.includes(needle)) {
    throw new Error(message);
  }
}

const cargo = read("crates/rustok-reactions-storefront/Cargo.toml");
const model = read("crates/rustok-reactions-storefront/src/model.rs");
const transport = read("crates/rustok-reactions-storefront/src/transport.rs");
const transportRuntime = transport.split("#[cfg(test)]", 1)[0];
const ui = read("crates/rustok-reactions-storefront/src/ui/leptos.rs");

for (const forbidden of ["rustok-forum", "rustok-blog", "rustok-reactions ="]) {
  requireAbsent(
    cargo,
    forbidden,
    `Reactions storefront must not depend on producer/private owner crate: ${forbidden}`,
  );
}

for (const field of ["source", "kind", "subject_id", "subject_revision"]) {
  requireContains(model, `pub ${field}`, `ReactionSubjectUiRef must expose ${field}`);
}
requireContains(
  model,
  "valid_segment",
  "ReactionSubjectUiRef must enforce the neutral source/kind segment contract",
);
requireContains(
  model,
  "subject_id.is_nil()",
  "ReactionSubjectUiRef must reject nil subject UUIDs",
);
requireContains(
  model,
  "revision.to_string() != subject_revision",
  "ReactionSubjectUiRef must require a canonical positive revision string",
);

for (const operation of ["reactionSnapshot", "applyReaction"]) {
  requireContains(transportRuntime, operation, `Missing neutral GraphQL operation ${operation}`);
}
for (const forbidden of ["tenantId", "actorId"]) {
  requireAbsent(
    transportRuntime,
    forbidden,
    `Storefront GraphQL input must not accept ${forbidden}`,
  );
}
requireContains(
  transportRuntime,
  "command_id: Uuid::new_v4()",
  "Each UI write must create a fresh owner command identity",
);
requireContains(
  ui,
  "set_refresh_nonce.update",
  "Every UI write attempt must reload canonical owner state",
);
requireAbsent(
  ui,
  "set_count",
  "UI must not maintain a shadow aggregate count",
);

console.log("reactions storefront UI source boundary: ok");
