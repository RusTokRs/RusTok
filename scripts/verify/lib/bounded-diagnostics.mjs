// Bounded-diagnostics policy for port and owner boundaries.
//
// The platform rule is one sentence: at a *port boundary* the caller records the shape of the
// `PortContext` it was handed and the shape of the owner failure it received, never their values.
// Identity, actor, channel, locale, causation, trace, idempotency, and deadline are logged as
// lengths, presence flags, counts, and typed labels; owner failure text is logged as a length plus
// a variant label. The owner service itself may still log its own identity fields under
// `docs/standards/logging.md`; the boundary may not, because it would re-log another owner's data.
//
// Two failure modes follow from that rule, and this module exists to make both of them loud:
//
//   1. *Leaking code.* A call site still interpolates `%context.tenant_id`, `?context.actor`, or
//      `internal_message = %error.message`, so tenant identity, actor identity, or third-party
//      error text lands in the log. `forbidRawContextLogging` names the line.
//
//   2. *Leaking guard.* A verifier written before the hardening still *requires* those raw fields,
//      so the correct implementation reads as a regression and the raw form reads as compliance.
//      That is worse than a missing check: it actively argues for the defect. Every guard that used
//      to call `require(source, 'tenant_id = %context.tenant_id')` should call
//      `forbidRawContextLogging` instead and require the bounded fact fields the source now emits.
//
// The patterns below are deliberately narrow. They match a `field = %value` / `field = ?value`
// *tracing directive*, not the words "tenant_id" or "error" anywhere in a file, because a guard that
// over-fires gets disabled and protects nothing.
//
// Deliberately not listed: `correlation_id` and `deadline_ms`. The per-call correlation id is a
// system-generated join key, never caller content, and several owner slices keep it on purpose for
// event joining (see `crates/modules/rustok-payment/docs/checkout-execution-local-context.md`);
// slices that prefer the shape form emit `correlation_id_length` instead, and both are compliant.
// The deadline is a bounded integer the platform itself computed. Everything else on a
// `PortContext` is caller-supplied text or identity, so it is listed.

/**
 * Raw interpolation directives that must never appear in a boundary log line.
 *
 * `%` and `?` are the two `tracing` sigils that render a value; a bare `field = value` is a
 * structured field whose meaning depends on the macro, so it is not listed here — the callers that
 * forbid it spell it out locally.
 *
 * Every rule is matched per call so a shared list never leaks `lastIndex` between files.
 */
export const RAW_CONTEXT_LOG_RULES = [
  {
    id: "tenant-value",
    pattern: /\btenant_id\s*=\s*[%?][\w.:]+/,
    reason: "tenant identity value",
  },
  {
    id: "actor-value",
    pattern: /\bactor\s*=\s*\?context\.actor\b/,
    reason: "actor identity value",
  },
  {
    id: "actor-id-value",
    pattern: /\bactor_id\s*=\s*[%?]/,
    reason: "actor identity value",
  },
  {
    id: "user-id-value",
    pattern: /\buser_id\s*=\s*[%?]/,
    reason: "user identity value",
  },
  {
    id: "channel-value",
    pattern: /\bchannel\s*=\s*\?context\.channel\b/,
    reason: "channel identity value",
  },
  {
    id: "locale-value",
    pattern: /\blocale\s*=\s*%context\.locale\b/,
    reason: "locale value",
  },
  {
    id: "causation-value",
    pattern: /\bcausation_id\s*=\s*\?context\.causation_id\b/,
    reason: "causation identity value",
  },
  {
    id: "traceparent-value",
    pattern: /\btraceparent\s*=\s*\?context\.traceparent\b/,
    reason: "traceparent value",
  },
  {
    id: "idempotency-value",
    pattern: /\bidempotency_key\s*=\s*\?context\.idempotency_key\b/,
    reason: "idempotency key value",
  },
  {
    id: "owner-message",
    pattern: /\binternal_message\s*=\s*%[a-z_.]*error(?:\.message)?\b/,
    reason: "owner message text",
  },
  {
    id: "owner-message-field",
    pattern: /\bmessage\s*:\s*error\.(?:message|to_string\(\))\b/,
    reason: "owner message text",
  },
  {
    id: "owner-error-debug",
    pattern: /\berror\s*=\s*\?boundary_error\b/,
    reason: "owner error payload",
  },
];

/** Every raw directive in `source`, with the line that carries it. */
export function rawContextLogging(source) {
  if (typeof source !== "string" || source.length === 0) return [];
  const found = [];
  for (const rule of RAW_CONTEXT_LOG_RULES) {
    const global = new RegExp(rule.pattern.source, "g");
    for (let match = global.exec(source); match !== null; match = global.exec(source)) {
      found.push({
        rule,
        index: match.index,
        line: lineAt(source, match.index),
        text: match[0],
      });
      if (match.index === global.lastIndex) global.lastIndex += 1;
    }
  }
  return found.sort((left, right) => left.index - right.index);
}

/**
 * Push one failure per raw directive, naming the line.
 *
 * The message carries the offending text and the reason, so the fix is obvious without opening the
 * guard and re-reading it.
 */
export function forbidRawContextLogging(failures, source, label) {
  for (const { rule, line, text } of rawContextLogging(source)) {
    failures.push(`${label}:${line}: raw ${rule.reason} in a log line (${text})`);
  }
  return failures;
}

/** Require one or more bounded fact fields, reporting the first missing one per field. */
export function requireFactFields(failures, source, fields, label) {
  for (const field of fields) {
    if (!source.includes(field)) failures.push(`${label}: missing bounded fact ${field}`);
  }
  return failures;
}

/**
 * The one-line policy check: bounded facts present, raw values absent.
 *
 * Callers that assert more (ordering, severity branches) keep doing that themselves.
 */
export function assertBoundedDiagnostics(failures, source, { facts = [], label }) {
  requireFactFields(failures, source, facts, label);
  forbidRawContextLogging(failures, source, label);
  return failures;
}

function lineAt(source, index) {
  let line = 1;
  for (let cursor = 0; cursor < index; cursor += 1) {
    if (source.charCodeAt(cursor) === 10) line += 1;
  }
  return line;
}
