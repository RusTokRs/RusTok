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
  // The server renders every `[data-fly-selected-component-input]` with the component its form
  // was rendered for, and the picker with the option that names it. That is the target until
  // something is selected in the browser, and the target an emptied selection returns to: the
  // adapter starts with no selection, so writing its selection unconditionally would erase the
  // rendered component id and leave the forms with no component to act on. The rendered value
  // is remembered here because assigning `value` to a hidden input rewrites its attribute.
  const __flyRenderedTargets = new WeakMap();
  const __flyRenderedTarget = (control) => {
    if (!__flyRenderedTargets.has(control)) __flyRenderedTargets.set(control, control.value);
    return __flyRenderedTargets.get(control);
  };
  const syncSelectionControls = () => {
    const selected = adapter.selectedId || "";
    for (const input of adapter.root.querySelectorAll('[data-fly-selected-component-input]')) {
      const rendered = __flyRenderedTarget(input);
      input.value = selected || rendered;
    }
    for (const picker of adapter.root.querySelectorAll('[data-fly-component-picker]')) {
      if (!(picker instanceof HTMLSelectElement)) continue;
      const target = selected || __flyRenderedTarget(picker);
      // A component the picker does not list keeps the option that is shown.
      if (Array.from(picker.options).some((option) => option.value === target)) {
        picker.value = target;
      }
    }
  };
  syncSelectionControls();
  adapter.root.addEventListener(
    "fly:select",
    () => queueMicrotask(syncSelectionControls),
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
  // A preview answer carries the document the server rendered. The panel's frame shows it as its
  // own document: `srcdoc` is a separate browsing context, but it is not a separate policy — it
  // inherits this document's, which is why the panel labels the result. A host that serves the
  // preview as its own response points the frame at it instead, and this binding does nothing.
  const __flyApplyPreview = (adapter, detail) => {
    if (detail.current === false) return;
    const persistence = detail.result?.persistence;
    if (!__flyObject(persistence) || persistence.capability !== "preview") return;
    const html = persistence.result?.html;
    if (typeof html !== "string" || !html) return;
    const frame = adapter.root.querySelector("[data-page-builder-server-preview-frame]");
    if (!(frame instanceof HTMLIFrameElement)) return;
    // A frame that already names a document by address must not be given one here: `srcdoc`
    // takes precedence over `src`, so writing it would replace the host's own document with an
    // inherited one — the exact defect the address exists to avoid. The panel reveals itself as
    // soon as the address answered, so there is nothing left to do.
    const served = frame.hasAttribute("src");
    if (!served) frame.srcdoc = html;
    frame.hidden = false;
  };
  adapter.root.addEventListener("fly:browser-intent-response", (event) => {
    const detail = event.detail;
    if (!detail?.ok || !__flyObject(detail.result)) return;
    const token = detail.result.draft_token;
    if (typeof token === "string" && token) __flyWriteDraftRoute(token);
  }, __flyLifecycleOptions);
  adapter.root.addEventListener(
    "fly:browser-intent-response",
    (event) => __flyApplyPreview(adapter, event.detail ?? {}),
    __flyLifecycleOptions,
  );
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
    fn the_bootstrap_never_overwrites_a_document_the_frame_loads_by_address() {
        // `srcdoc` takes precedence over `src`, so a bootstrap that writes it unconditionally
        // would replace a host-served preview — the page's own document under the page's own
        // policy — with a document inheriting the console's. That is the defect the address
        // exists to avoid, so the guard is part of the contract, not an optimisation.
        let source = PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS;
        let start = source
            .find("const __flyApplyPreview")
            .expect("preview binding");
        let end = start + source[start..].find("adapter.root.addEventListener").expect("binding end");
        let binding = &source[start..end];
        assert!(
            binding.contains("frame.hasAttribute(\"src\")"),
            "the preview binding does not check the address before writing `srcdoc`: {binding}"
        );
        let guard = binding.find("if (!served)").expect("guarded write");
        let write = binding.find("frame.srcdoc = html").expect("write");
        assert!(guard < write, "{binding}");
        assert!(binding.contains("frame.hidden = false"), "{binding}");
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
        assert!(
            PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS
                .contains("Symbol.for(\"fly.browser.ssr.controls\")")
        );
        assert!(PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.contains("adapters: new WeakSet()"));
        assert!(PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.contains("fly:browser-ready"));
        assert!(
            PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS
                .contains("FlyBrowser?.bootstrap?.(__flyBrowserConfig)")
        );
        assert!(PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.contains("adapter.abortController?.signal"));
        assert!(
            !PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.contains(&format!("autoMount {} false", "==="))
        );
        assert!(!PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.contains("leptos"));
        assert!(!PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS.contains("dioxus"));
    }
}
