pub const PAGE_BUILDER_BROWSER_ADAPTER: &str = "fly_browser";
pub const PAGE_BUILDER_BROWSER_SCRIPT_TYPE: &str = "module";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageBuilderBrowserModuleOptions {
    pub nonce: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBuilderBrowserModuleDescriptor {
    pub script_type: &'static str,
    pub adapter: &'static str,
    pub nonce: Option<String>,
    pub source: String,
}

pub const PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS: &str = r#"
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
const __flySsrControlStateKey = Symbol.for("fly.browser.ssr.controls");
const __flySsrControlState = globalThis[__flySsrControlStateKey] || {
  adapters: new WeakSet(),
  bind: null,
  listening: false,
};
globalThis[__flySsrControlStateKey] = __flySsrControlState;
const __flyBindSsrAdapter = (adapter) => {
  if (!adapter || __flySsrControlState.adapters.has(adapter)) return adapter;
  if (!(adapter.root instanceof Element)) return null;
  const __flyLifecycleSignal = adapter.abortController?.signal;
  if (!(__flyLifecycleSignal instanceof AbortSignal) || __flyLifecycleSignal.aborted) {
    return null;
  }
  const __flyLifecycleOptions = { signal: __flyLifecycleSignal };
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
  __flySsrControlState.adapters.add(adapter);
  return adapter;
};
__flySsrControlState.bind = __flyBindSsrAdapter;
if (!__flySsrControlState.listening) {
  document.addEventListener("fly:browser-ready", (event) => {
    __flySsrControlState.bind?.(event.detail?.adapter);
  });
  __flySsrControlState.listening = true;
}
const __flyAdapters = globalThis.FlyBrowser?.bootstrap?.(__flyBrowserConfig) || [];
for (const adapter of __flyAdapters) __flyBindSsrAdapter(adapter);
"#;

pub fn page_builder_browser_module(
    config_json: &str,
    adapter_js: &str,
    options: PageBuilderBrowserModuleOptions,
) -> PageBuilderBrowserModuleDescriptor {
    let config = escape_browser_config_for_inline_script(config_json);
    let source = [
        format!("globalThis.__FLY_BROWSER_CONFIG__ = Object.freeze({config});"),
        adapter_js.to_string(),
        PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.to_string(),
    ]
    .join("\n");

    PageBuilderBrowserModuleDescriptor {
        script_type: PAGE_BUILDER_BROWSER_SCRIPT_TYPE,
        adapter: PAGE_BUILDER_BROWSER_ADAPTER,
        nonce: normalize_script_nonce(options.nonce),
        source,
    }
}

fn normalize_script_nonce(nonce: Option<String>) -> Option<String> {
    nonce.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

pub fn escape_browser_config_for_inline_script(json: &str) -> String {
    json.replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_descriptor_orders_config_adapter_and_host_contract() {
        let module = page_builder_browser_module(
            r#"{"autoMount":false}"#,
            "export class FlyBrowserAdapter {}",
            PageBuilderBrowserModuleOptions {
                nonce: Some("  csp-nonce  ".to_string()),
            },
        );
        let config = module
            .source
            .find("globalThis.__FLY_BROWSER_CONFIG__")
            .expect("config source");
        let adapter = module
            .source
            .find("export class FlyBrowserAdapter")
            .expect("adapter source");
        let host = module
            .source
            .find("const __flyDraftQueryKey")
            .expect("host source");
        assert!(config < adapter);
        assert!(adapter < host);
        assert_eq!(module.script_type, "module");
        assert_eq!(module.adapter, "fly_browser");
        assert_eq!(module.nonce.as_deref(), Some("csp-nonce"));
    }

    #[test]
    fn blank_script_nonce_is_omitted() {
        let module = page_builder_browser_module(
            "{}",
            "export class FlyBrowserAdapter {}",
            PageBuilderBrowserModuleOptions {
                nonce: Some("   ".to_string()),
            },
        );
        assert_eq!(module.nonce, None);
    }

    #[test]
    fn config_cannot_close_the_inline_script() {
        let escaped = escape_browser_config_for_inline_script(
            r#"{"endpoint":"</script><script>alert(1)</script>","token":"a&b"}"#,
        );
        assert!(!escaped.contains("</script>"));
        assert!(!escaped.contains('&'));
        assert!(escaped.contains("\\u003c/script\\u003e"));
    }

    #[test]
    fn late_manual_mount_contract_is_framework_neutral() {
        assert!(PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS
            .contains("Symbol.for(\"fly.browser.ssr.controls\")"));
        assert!(PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.contains("adapters: new WeakSet()"));
        assert!(PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.contains("fly:browser-ready"));
        assert!(PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS
            .contains("FlyBrowser?.bootstrap?.(__flyBrowserConfig)"));
        assert!(PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS
            .contains("adapter.abortController?.signal"));
        assert!(!PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.contains("autoMount === false"));
        assert!(!PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.contains("leptos"));
        assert!(!PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.contains("dioxus"));
    }
}
