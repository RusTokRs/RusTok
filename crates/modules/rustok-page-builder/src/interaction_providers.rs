//! Host-owned interaction provider catalog.
//!
//! Fly ships the provider *contract* — `InteractionCapabilityRegistry`,
//! `evaluate_runtime_publish_gate_with_capabilities`, and `data-fly-form-provider` markers that
//! name the provider a form belongs to — and deliberately refuses to guess which providers a
//! deployment runs. This module is the RusTok host's answer:
//!
//! * every provider interaction this repository serves is registered once, here, and the publish
//!   gates consume that catalog, so a provider form the host cannot submit is a blocking
//!   diagnostic instead of a form that silently posts back to the page;
//! * [`HostFormProviderEndpoints`] is the matching runtime half — the resolver the storefront
//!   passes into Fly materialization so a registered provider form gets a native `action`.
//!
//! Provider identities are mirrored from the modules that serve them instead of imported: the
//! page builder stays buildable standalone (the forms module pulls a server stack this reference
//! module must not require), and `scripts/verify/verify-fly-interaction-capabilities.mjs` fails
//! when the identifiers or the route template drift apart.

use crate::{
    evaluate_runtime_publish_gate_with_capabilities, RuntimeContextScenario,
    RuntimePublishGateEvaluation, RuntimePublishGatePolicy,
};
use fly::{
    ContextValueKind, FormProviderEndpointResolver, InteractionCapabilityDefinition,
    InteractionCapabilityKind, InteractionCapabilityPolicy, InteractionCapabilityRegistry,
    InteractionRuntimeTarget, MissingInteractionCapabilityPolicy, ProjectDocument,
};
use serde_json::Value;

/// Provider id rendered into `data-fly-form-provider` for forms served by `rustok-forms`.
pub const FORMS_PROVIDER_ID: &str = "forms";

/// Provider operation (`flyForm.action`) bound to that module's submission intake.
pub const FORMS_SUBMIT_OPERATION: &str = "submit";

/// URL-encoded intake route template served by `rustok-forms`; `{form_id}` is the Fly form id.
///
/// Mirrors `SUBMIT_URLENCODED_ROUTE` in `crates/modules/rustok-forms/src/controllers.rs`.
pub const FORMS_SUBMIT_ENDPOINT_TEMPLATE: &str = "/api/forms/{form_id}/submit/form";

/// Registers every provider interaction this host serves.
///
/// The ids are compile-time literals, so a registration failure is a programming error — the same
/// invariant `fly::RegistrySet::with_builtins` asserts for its built-ins.
pub fn host_interaction_capability_registry() -> InteractionCapabilityRegistry {
    let mut registry = InteractionCapabilityRegistry::default();
    // `input_kind` is `Any` on purpose: `flyForm.input` is an opaque provider descriptor, and the
    // forms intake payload is the flat object of submitted fields, not that descriptor.
    registry
        .register(InteractionCapabilityDefinition {
            provider: FORMS_PROVIDER_ID.to_string(),
            operation: FORMS_SUBMIT_OPERATION.to_string(),
            kind: InteractionCapabilityKind::Form,
            runtime: InteractionRuntimeTarget::Server,
            input_kind: ContextValueKind::Any,
            description:
                "Native HTML form intake served by the rustok-forms module (`POST /api/forms/{form_id}/submit/form`)."
                    .to_string(),
        })
        .expect("host interaction provider ids are valid and unique");
    registry
}

/// Capability policy for the catalog above.
///
/// A provider form whose provider is not registered cannot be submitted by this host at all, so it
/// blocks the gate — the authoring surfaces consult it for save and publish intents, and the
/// message names the missing capability so the author can replace or remove the form. A provider
/// action is a button that still renders and simply has no handler in this repository (see the Fly
/// README's custom-action boundary), so it warns without blocking; hosts that run their own
/// providers register them here or relax the policy through
/// [`evaluate_host_runtime_publish_gate_with_policy`].
pub fn host_interaction_capability_policy() -> InteractionCapabilityPolicy {
    InteractionCapabilityPolicy {
        provider_actions: MissingInteractionCapabilityPolicy::Warn,
        provider_forms: MissingInteractionCapabilityPolicy::Error,
    }
}

/// Evaluates the publish gate with the host provider catalog applied.
pub fn evaluate_host_runtime_publish_gate(
    document: &ProjectDocument,
    current_context: Option<&Value>,
    scenarios: &[RuntimeContextScenario],
    policy: &RuntimePublishGatePolicy,
) -> RuntimePublishGateEvaluation {
    evaluate_host_runtime_publish_gate_with_policy(
        document,
        current_context,
        scenarios,
        policy,
        host_interaction_capability_policy(),
    )
}

/// Evaluates the publish gate with the host catalog and an explicit capability policy.
pub fn evaluate_host_runtime_publish_gate_with_policy(
    document: &ProjectDocument,
    current_context: Option<&Value>,
    scenarios: &[RuntimeContextScenario],
    policy: &RuntimePublishGatePolicy,
    capability_policy: InteractionCapabilityPolicy,
) -> RuntimePublishGateEvaluation {
    evaluate_runtime_publish_gate_with_capabilities(
        document,
        current_context,
        scenarios,
        policy,
        &host_interaction_capability_registry(),
        capability_policy,
    )
}

/// Form endpoint resolver for the providers this host serves.
///
/// Pass this into `render_page_with_runtime_context_with_providers` (or the materialization
/// variant) so a `forms`/`submit` provider form renders `action="/api/forms/{id}/submit/form"`
/// — POST, URL-encoded — instead of posting back to the page URL.
#[derive(Debug, Clone, Copy, Default)]
pub struct HostFormProviderEndpoints;

impl FormProviderEndpointResolver for HostFormProviderEndpoints {
    fn form_endpoint(&self, provider: &str, operation: &str, form_id: &str) -> Option<String> {
        if provider.trim() != FORMS_PROVIDER_ID || operation.trim() != FORMS_SUBMIT_OPERATION {
            return None;
        }
        let form_id = form_id.trim();
        if form_id.is_empty() {
            return None;
        }
        Some(FORMS_SUBMIT_ENDPOINT_TEMPLATE.replace("{form_id}", &encode_path_segment(form_id)))
    }
}

/// Percent-encodes one path segment.
///
/// Fly form ids are author-supplied, so the endpoint must not rely on their charset even though
/// the editor validates them; unreserved characters stay readable.
fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
