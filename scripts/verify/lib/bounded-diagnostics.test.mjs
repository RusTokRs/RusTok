import assert from "node:assert/strict";
import test from "node:test";

import {
  RAW_CONTEXT_LOG_RULES,
  assertBoundedDiagnostics,
  forbidRawContextLogging,
  rawContextLogging,
  requireFactFields,
} from "./bounded-diagnostics.mjs";

test("finds each raw context directive form and reports its line", () => {
  const source = [
    "fn log(context: &PortContext) {",
    "    tracing::warn!(",
    "        tenant_id = %context.tenant_id,",
    "        actor = ?context.actor,",
    "        channel = ?context.channel,",
    "        locale = %context.locale,",
    "        owner = \"rustok_cart\",",
    "    );",
    "}",
  ].join("\n");

  const found = rawContextLogging(source);
  assert.deepEqual(
    found.map(({ line, text }) => [line, text]),
    [
      [3, "tenant_id = %context.tenant_id"],
      [4, "actor = ?context.actor"],
      [5, "channel = ?context.channel"],
      [6, "locale = %context.locale"],
    ],
  );
});

test("a correlation id is a join key, not caller content", () => {
  const raw = "correlation_id = %context.correlation_id,\n";
  assert.deepEqual(rawContextLogging(raw), []);
});

test("finds owner message text but not a bounded length fact", () => {
  const raw = ["internal_message = %error.message,", "message: error.message,", "internal_message = %error,"].join(
    "\n",
  );
  const bounded = "error_message_length = error_facts.message_length,\n";

  assert.deepEqual(
    rawContextLogging(raw).map(({ rule }) => rule.id),
    ["owner-message", "owner-message-field", "owner-message"],
  );
  assert.equal(rawContextLogging(bounded).length, 0);
});

test("a bare field name without a sigil is not a raw directive", () => {
  const source = "tenant_id_length = facts.tenant_id_length,\nlet tenant_id = parse(context)?;\n";
  assert.deepEqual(rawContextLogging(source), []);
});

test("the bounded fact projection itself is never flagged", () => {
  const source = [
    "tenant_id_length = context_facts.tenant_id_length,",
    "actor_kind = context_facts.actor_kind,",
    "correlation_id_length = context_facts.correlation_id_length,",
    "channel_present = context_facts.channel_present,",
    "deadline_ms = ?context_facts.deadline_ms,",
    "error_message_length = error_facts.message_length,",
  ].join("\n");
  assert.deepEqual(rawContextLogging(source), []);
});

test("forbidden directives carry the reason and the line", () => {
  const failures = [];
  forbidRawContextLogging(failures, "a\nb\n    error = ?boundary_error\n", "cart stage");
  assert.equal(failures.length, 1);
  assert.match(failures[0], /^cart stage:3: raw owner error payload in a log line \(error = \?boundary_error\)$/);
});

test("bounded fact requirement reports each missing field once", () => {
  const failures = [];
  requireFactFields(failures, "tenant_id_length\n", ["tenant_id_length", "actor_kind"], "facts");
  assert.deepEqual(failures, ["facts: missing bounded fact actor_kind"]);
});

test("assertBoundedDiagnostics combines both halves of the policy", () => {
  const clean = [];
  assertBoundedDiagnostics(clean, "tenant_id_length, actor_kind", {
    facts: ["tenant_id_length", "actor_kind"],
    label: "bounded",
  });
  assert.deepEqual(clean, []);

  const dirty = [];
  assertBoundedDiagnostics(dirty, "tenant_id = %context.tenant_id", {
    facts: ["tenant_id_length"],
    label: "dirty",
  });
  assert.equal(dirty.length, 2);
  assert.match(dirty[0], /missing bounded fact tenant_id_length/);
  assert.match(dirty[1], /raw tenant identity value/);
});

test("every rule is anchored to a tracing directive, not to a bare word", () => {
  const prose = "The tenant_id field flows through context and the error is logged elsewhere.";
  assert.deepEqual(rawContextLogging(prose), []);

  for (const rule of RAW_CONTEXT_LOG_RULES) {
    assert.match(rule.id, /^[a-z][a-z-]*$/, "rule ids are stable and greppable");
    assert.match(rule.reason, /\S/, "every rule explains why the value is forbidden");
    assert.equal(rule.pattern.flags.includes("g"), false, "rules are compiled per call site");
  }
});
