#!/usr/bin/env node
// Compile-free guardrails for AI router provider fallback/candidate evidence.

import { existsSync, readFileSync } from "node:fs";

const failures = [];
function fail(message) { failures.push(message); }
function read(path) { return readFileSync(path, "utf8"); }
function assertExists(path) { if (!existsSync(path)) fail(`${path}: missing required file`); }
function assertIncludes(text, needle, label) { if (!text.includes(needle)) fail(`${label}: missing ${needle}`); }
function assertOrdered(text, needles, label) {
  let cursor = -1;
  for (const needle of needles) {
    const next = text.indexOf(needle, cursor + 1);
    if (next === -1) fail(`${label}: missing ordered marker ${needle}`);
    cursor = next === -1 ? cursor : next;
  }
}

const files = {
  router: "crates/rustok-ai/src/router.rs",
  plan: "crates/rustok-ai/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
};
for (const file of Object.values(files)) assertExists(file);

const router = read(files.router);
const plan = read(files.plan);
const registry = read(files.registry);

assertOrdered(router, [
  "pub enum RouterCandidateStatus",
  "pub struct RouterCandidateDecision",
  "pub fn explain_provider_candidates",
  "fn provider_candidate_status",
], "router candidate evidence API");

for (const marker of [
  "MissingCapability",
  "NotInTaskAllowList",
  "TaskDeniedByProviderPolicy",
  "NotInProviderAllowList",
  "MissingRequiredActorRole",
  "Provider candidate `{}` status `{}`: {}",
  "explain_provider_candidates_records_all_policy_reasons",
  "resolve_decision_trace_includes_candidate_statuses",
]) assertIncludes(router, marker, "router fallback/candidate policy");

assertIncludes(plan, "ai_router_policy_evidence_expanded", "AI implementation checkpoint");
assertIncludes(plan, "scripts/verify/verify-ai-router-policy.mjs", "AI implementation guardrail list");
assertIncludes(registry, "verify-ai-router-policy.mjs", "central registry AI evidence");

if (failures.length > 0) {
  console.error("AI router policy verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("AI router policy verification passed");
