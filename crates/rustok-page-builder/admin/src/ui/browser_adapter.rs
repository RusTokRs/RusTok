use fly_browser::{BrowserAdapterConfig, FLY_BROWSER_ADAPTER_JS};
use leptos::prelude::*;

const SSR_CONTROL_BOOTSTRAP_JS: &str = r#"
const __flyDraftQueryKey = "fly_draft";
const __flyObject = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
const __flyFormPayload = (form) => {
  const payload = {};
  for (const [name, value] of new FormData(form).entries()) {
    if (Object.hasOwn(payload, name)) {
      payload[name] = Array.isArray(payload[name])
        ? [...payload[name], value]
        : [payload[name], value];
    } else {
      payload[name] = value;
    }
  }
  for (const checkbox of form.querySelectorAll('input[type="checkbox"][name]')) {
    payload[checkbox.name] = checkbox.checked;
  }
  for (const number of form.querySelectorAll('input[type="number"][name]')) {
    if (number.value === "") {
      delete payload[number.name];
    } else if (Number.isFinite(Number(number.value))) {
      payload[number.name] = Number(number.value);
    }
  }
  return payload;
};
const __flyDraftFromRoute = () => {
  try {
    return new URL(globalThis.location.href).searchParams.get(__flyDraftQueryKey);
  } catch (_) {
    return null;
  }
};
const __flyWriteDraftRoute = (token) => {
  if (typeof token !== "string" || !token) return;
  try {
    const url = new URL(globalThis.location.href);
    if (url.searchParams.get(__flyDraftQueryKey) === token) return;
    url.searchParams.set(__flyDraftQueryKey, token);
    globalThis.history.replaceState(globalThis.history.state, "", url);
  } catch (_) {
    // URL synchronization is progressive enhancement; the server endpoint still works without it.
  }
};
const __flyBrowserConfig = globalThis.__FLY_BROWSER_CONFIG__ || {};
const __flyAdapters = __flyBrowserConfig.autoMount === false
  ? []
  : globalThis.FlyBrowser?.mountAll(__flyBrowserConfig) || [];
for (const adapter of __flyAdapters) {
  const __flyLifecycleSignal = adapter.abortController?.signal;
  const __flyLifecycleOptions = __flyLifecycleSignal
    ? { signal: __flyLifecycleSignal }
    : undefined;
  const routeDraft = __flyDraftFromRoute();
  if (routeDraft && adapter.draftSession?.token !== routeDraft) {
    adapter.draftSession = { token: routeDraft, generation: null };
  }
  const updateSelectedInputs = () => {
    for (const input of adapter.root.querySelectorAll('[data-fly-selected-component-input]')) {
      input.value = adapter.selectedId || "";
    }
  };
  updateSelectedInputs();
  adapter.root.addEventListener(
    "fly:select",
    () => queueMicrotask(updateSelectedInputs),
    __flyLifecycleOptions,
  );
  adapter.root.addEventListener("change", (event) => {
    const picker = event.target instanceof Element
      ? event.target.closest("[data-fly-component-picker]")
      : null;
    if (!(picker instanceof HTMLSelectElement)) return;
    adapter.selectedId = picker.value || null;
    adapter.drawSelection();
    adapter.root.dispatchEvent(new CustomEvent("fly:select", {
      bubbles: true,
      detail: { componentId: adapter.selectedId, source: "ssr-inspector" },
    }));
  }, __flyLifecycleOptions);
  adapter.root.addEventListener("submit", (event) => {
    const form = event.target instanceof Element
      ? event.target.closest("form[data-fly-intent-form]")
      : null;
    if (!(form instanceof HTMLFormElement)) return;
    event.preventDefault();
    const intent = form.dataset.flyIntentForm;
    if (!intent) return;
    const payload = __flyFormPayload(form);
    if (!payload.component_id && adapter.selectedId) payload.component_id = adapter.selectedId;
    void adapter.emitIntent(intent, payload);
  }, __flyLifecycleOptions);
  adapter.root.addEventListener("fly:browser-intent-response", (event) => {
    const detail = event.detail;
    if (!detail?.ok || !__flyObject(detail.result)) return;
    const token = detail.result.draft_token;
    if (typeof token === "string" && token) __flyWriteDraftRoute(token);
  }, __flyLifecycleOptions);
}
"#;

fn browser_adapter_config_json(
    intent_endpoint: Option<String>,
    csrf_token: Option<String>,
) -> Result<String, serde_json::Error> {
    BrowserAdapterConfig {
        intent_endpoint,
        csrf_token,
        ..BrowserAdapterConfig::default()
    }
    .to_json()
}

/// Emits the standalone Fly browser bridge into a server-rendered Page Builder surface.
///
/// The script owns only DOM identity checks, bounded geometry overlays, pointer/keyboard forwarding,
/// progressive enhancement for ordinary SSR forms, and optional POST delivery to a
/// consumer-owned intent endpoint. Fly project state, commands, validation, rendering,
/// permissions, and persistence remain in Rust.
#[component]
pub fn PageBuilderBrowserAdapter(
    #[prop(optional_no_strip)] intent_endpoint: Option<String>,
    #[prop(optional_no_strip)] csrf_token: Option<String>,
) -> impl IntoView {
    #[cfg(feature = "browser-js")]
    {
        let config = browser_adapter_config_json(intent_endpoint, csrf_token)
            .map(|json| escape_json_for_script(&json))
            .unwrap_or_else(|_| "{}".to_string());
        let source = [
            format!("globalThis.__FLY_BROWSER_CONFIG__ = Object.freeze({config});"),
            FLY_BROWSER_ADAPTER_JS.to_string(),
            SSR_CONTROL_BOOTSTRAP_JS.to_string(),
        ]
        .join("\n");
        view! {
            <script
                type="module"
                data-fly-browser-adapter="fly_browser"
                inner_html=source
            ></script>
        }
        .into_any()
    }

    #[cfg(not(feature = "browser-js"))]
    {
        let _ = (intent_endpoint, csrf_token);
        view! { <span hidden data-fly-browser-adapter="disabled"></span> }.into_any()
    }
}

fn escape_json_for_script(json: &str) -> String {
    json.replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_browser::{
        DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS, DEFAULT_MAX_BROWSER_MESSAGE_BYTES,
    };

    #[test]
    fn adapter_asset_does_not_depend_on_wasm_runtime() {
        assert!(FLY_BROWSER_ADAPTER_JS.contains("class FlyBrowserAdapter"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-intent"));
        assert!(!FLY_BROWSER_ADAPTER_JS.contains("wasm_bindgen"));
    }

    #[test]
    fn browser_config_uses_the_public_javascript_contract() {
        let json = browser_adapter_config_json(
            Some("/admin/fly/intents".to_string()),
            Some("csrf-token".to_string()),
        )
        .expect("browser config");
        let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
        assert_eq!(value["intentEndpoint"], "/admin/fly/intents");
        assert_eq!(value["csrfToken"], "csrf-token");
        assert_eq!(
            value["maxMessageBytes"],
            DEFAULT_MAX_BROWSER_MESSAGE_BYTES
        );
        assert_eq!(
            value["maxGeometryComponents"],
            DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS
        );
        assert!(value.get("intent_endpoint").is_none());
        assert!(value.get("csrf_token").is_none());
    }

    #[test]
    fn public_adapter_bundle_emits_typed_accessible_resource_limits() {
        assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-resource-limit"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("message_bytes"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("geometry_components"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("aria-live"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("role"));
    }

    #[test]
    fn bootstrap_json_cannot_close_the_inline_script() {
        let escaped = escape_json_for_script(
            r#"{"endpoint":"</script><script>alert(1)</script>","token":"a&b"}"#,
        );
        assert!(!escaped.contains("</script>"));
        assert!(!escaped.contains('&'));
        assert!(escaped.contains("\\u003c/script\\u003e"));
    }

    #[test]
    fn ssr_bootstrap_submits_forms_and_synchronizes_draft_routes() {
        assert!(SSR_CONTROL_BOOTSTRAP_JS.contains("data-fly-intent-form"));
        assert!(SSR_CONTROL_BOOTSTRAP_JS.contains("__flyFormPayload"));
        assert!(SSR_CONTROL_BOOTSTRAP_JS.contains("delete payload[number.name]"));
        assert!(SSR_CONTROL_BOOTSTRAP_JS.contains("fly_draft"));
        assert!(SSR_CONTROL_BOOTSTRAP_JS.contains("history.replaceState"));
        assert!(SSR_CONTROL_BOOTSTRAP_JS.contains("data-fly-component-picker"));
        assert!(SSR_CONTROL_BOOTSTRAP_JS.contains("autoMount === false"));
    }

    #[test]
    fn ssr_bootstrap_listeners_follow_adapter_lifecycle() {
        assert!(SSR_CONTROL_BOOTSTRAP_JS.contains("adapter.abortController?.signal"));
        assert!(SSR_CONTROL_BOOTSTRAP_JS.contains("__flyLifecycleOptions"));
        assert!(SSR_CONTROL_BOOTSTRAP_JS.contains("signal: __flyLifecycleSignal"));
        assert_eq!(SSR_CONTROL_BOOTSTRAP_JS.matches("__flyLifecycleOptions").count(), 5);
    }
}
