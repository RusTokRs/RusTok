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
  "revision == 0",
  "ReactionSubjectUiRef must reject non-positive revisions",
);

for (const operation of ["reactionSnapshot", "applyReaction"]) {
  requireContains(transport, operation, `Missing neutral GraphQL operation ${operation}`);
}
for (const forbidden of ["tenantId", "actorId"]) {
  requireAbsent(
    transport,
    forbidden,
    `Storefront GraphQL input must not accept ${forbidden}`,
  );
}
requireContains(
  transport,
  "command_id: Uuid::new_v4()",
  "Each UI write must create a fresh owner command identity",
);
requireContains(
  ui,
  "set_refresh_nonce.update",
  "Successful writes must reload canonical owner state",
);
requireAbsent(
  ui,
  "set_count",
  "UI must not maintain a shadow aggregate count",
);

console.log("reactions storefront UI source boundary: ok");
