// Shared source model for the cart checkout port.
//
// Five verifiers read `crates/modules/rustok-cart/src/checkout_snapshot.rs` and each one used to
// repeat the same `between`/`from` block isolation and the same anchor literals. When the bounded
// diagnostics landed, all five kept the old anchors and the old raw-field requirements, so the
// correct source read as a regression. The block model lives here now: one place to update when the
// file is restructured, and no guard carries its own copy of a string index.
//
// Each block is isolated by its definition line and ends at the next definition, so a `#[cfg(...)]`
// attribute or a doc comment between items never shifts an anchor.

export const CART_CHECKOUT_SOURCE =
  "crates/modules/rustok-cart/src/checkout_snapshot.rs";

const BLOCKS = [
  ["constants", null, "fn cart_checkout_context_facts("],
  ["contextFactsStruct", "struct CartCheckoutContextFacts {", "struct CartCheckoutPortErrorFacts {"],
  ["portErrorFactsStruct", "struct CartCheckoutPortErrorFacts {", "struct CartCheckoutServiceErrorFacts {"],
  ["serviceErrorFactsStruct", "struct CartCheckoutServiceErrorFacts {", "fn cart_checkout_context_facts("],
  ["contextFacts", "fn cart_checkout_context_facts(", "fn cart_checkout_port_error_facts("],
  ["portErrorFacts", "fn cart_checkout_port_error_facts(", "fn cart_checkout_service_error_facts("],
  ["serviceErrorFacts", "fn cart_checkout_service_error_facts(", "fn log_cart_checkout_port_error("],
  ["portErrorLog", "fn log_cart_checkout_port_error(", "fn log_cart_checkout_service_error("],
  ["serviceErrorLog", "fn log_cart_checkout_service_error(", "fn in_process_cart_checkout_port("],
  ["operationStart", "fn log_cart_checkout_operation_start(", "fn require_cart_checkout_read_admission("],
  ["admission", "fn require_cart_checkout_read_admission(", "#[async_trait]"],
  ["portImpl", "#[async_trait]\nimpl CartCheckoutPort for InProcessCartCheckoutPort", "fn map_cart_checkout_local_port_error("],
  ["localMapper", "fn map_cart_checkout_local_port_error(", "fn map_cart_checkout_service_error("],
  ["serviceMapper", "fn map_cart_checkout_service_error(", "fn validate_prepare_input("],
  ["prepareInput", "fn validate_prepare_input(", "fn parse_tenant_id("],
  ["parser", "fn parse_tenant_id(", "fn snapshot_from_cart("],
  ["publicErrorMapper", "fn cart_error_to_port_error(", "#[cfg(test)]"],
];

const OPERATIONS = [
  ["prepare", "async fn prepare_checkout(", "async fn read_checkout_snapshot("],
  ["readSnapshot", "async fn read_checkout_snapshot(", "async fn complete_checkout("],
  ["complete", "async fn complete_checkout(", "async fn release_checkout("],
  ["release", "async fn release_checkout(", null],
];

/**
 * Isolate every block the cart checkout verifiers assert on.
 *
 * `read` is the caller's file reader, `failures` its failure sink, so a missing anchor is reported
 * with that verifier's own label instead of throwing.
 */
export function createCartCheckoutSource({ read, failures, label = "cart checkout source" }) {
  const source = read(CART_CHECKOUT_SOURCE);
  const blocks = { source };

  for (const [name, start, end] of BLOCKS) {
    if (start === null) {
      blocks[name] = source.slice(0, source.indexOf(end));
      continue;
    }
    blocks[name] = between(source, start, end, `${label} ${name}`, failures);
  }

  for (const [name, start, end] of OPERATIONS) {
    if (end === null) {
      const startIndex = blocks.portImpl.indexOf(start);
      if (startIndex < 0) {
        failures.push(`${label} ${name}: unable to isolate source block`);
        blocks[name] = "";
      } else {
        blocks[name] = blocks.portImpl.slice(startIndex);
      }
      continue;
    }
    blocks[name] = between(blocks.portImpl, start, end, `${label} ${name}`, failures);
  }

  return blocks;
}

function between(content, start, end, label, failures) {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return content.slice(startIndex, endIndex);
}

// The bounded fact contract the cart checkout port publishes. Slices that assert the diagnostics
// require these names instead of the raw `PortContext` and `CartError` values they replaced; a new
// fact is added here once and every verifier picks it up.

export const CART_CONTEXT_FACT_FIELDS = [
  "correlation_id_length",
  "tenant_id_length",
  "actor_kind",
  "actor_id_length",
  "claim_count",
  "role_count",
  "channel_present",
  "channel_length",
  "locale_length",
  "causation_id_present",
  "causation_id_length",
  "traceparent_present",
  "traceparent_length",
  "idempotency_key_present",
  "idempotency_key_length",
  "deadline_ms",
];

export const CART_PORT_ERROR_FACT_FIELDS = [
  "error_kind",
  "error_code_length",
  "message_present",
  "message_length",
  "retryable",
];

export const CART_PORT_ERROR_LOG_FIELDS = [
  "error_kind",
  "error_code_length",
  "error_message_present",
  "error_message_length",
  "retryable",
];

export const CART_SERVICE_ERROR_FACT_FIELDS = [
  "error_variant",
  "text_field_count",
  "text_total_length",
  "uuid_field_count",
  "uuid_non_nil_count",
  "opaque_payload_present",
];
