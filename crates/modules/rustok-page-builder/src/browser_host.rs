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
const __flyDraftUrl = (token) => {
  try {
    const url = new URL(globalThis.location.href);
    if (typeof token === "string" && token) url.searchParams.set(__flyDraftQueryKey, token);
    return url;
  } catch (_) {
    return null;
  }
};
const __flyWriteDraftRoute = (token) => {
  if (typeof token !== "string" || !token) return false;
  try {
    const url = __flyDraftUrl(token);
    if (!url) return false;
    if (__flyDraftFromRoute() !== token) {
      globalThis.history.replaceState(globalThis.history.state, "", url);
    }
    return __flyDraftFromRoute() === token;
  } catch (_) {
    return false;
  }
};
// Never let a canvas post into a session different from the one rendered by
// the server. Keep the cached token intact and offer a same-origin recovery
// link; the server still checks session owner and page on the next render.
const __flyStopForDraftRecovery = (adapter, token, showNotice = true) => {
  adapter.postIntents = false;
  adapter.intentEndpoint = null;
  adapter.stop();
  const root = adapter.root;
  root.inert = true;
  if (!showNotice) return;
  if (root.previousElementSibling?.hasAttribute("data-fly-draft-recovery")) return;
  const notice = document.createElement("div");
  notice.setAttribute("data-fly-draft-recovery", "true");
  notice.setAttribute("role", "alert");
  notice.setAttribute("class", "rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive");
  const message = document.createElement("p");
  message.textContent = root.dataset.flyDraftRecoveryMessage ||
    "The editor does not match its draft session. Editing is stopped. Restore the draft before continuing; it may expire.";
  notice.append(message);
  const url = __flyDraftUrl(token);
  if (url) {
    const link = document.createElement("a");
    link.href = url.toString();
    link.textContent = root.dataset.flyDraftRecoveryLink || "Restore draft or reload this address";
    link.setAttribute("class", "mt-2 inline-block font-medium underline");
    notice.append(link);
  }
  root.parentNode?.insertBefore(notice, root);
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
  const strictDraft = adapter.root.dataset.flyDraftContract === "rendered";
  // Bind responses to the page and route that owned this canvas. A request may
  // finish after client-side navigation even if it is the newest request for
  // this adapter. Never write its token into a different page's route.
  let boundUrl;
  try { boundUrl = new URL(globalThis.location.href); } catch (_) { boundUrl = null; }
  let boundDraft = routeDraft;
  if (strictDraft) {
    const rendered = adapter.root.dataset.flyRenderedDraftToken || null;
    const rawGeneration = adapter.root.dataset.flyRenderedDraftGeneration;
    const generation = typeof rawGeneration === "string" && /^[1-9][0-9]*$/.test(rawGeneration)
      ? Number(rawGeneration) : NaN;
    if (rendered) {
      if (routeDraft !== rendered || !Number.isSafeInteger(generation)) {
        __flyStopForDraftRecovery(adapter, rendered);
        return adapter;
      }
      // The generation belongs to the SSR snapshot, not to independently cached state.
      adapter.draftSession = { token: rendered, generation };
    } else if (routeDraft !== null || rawGeneration !== undefined) {
      __flyStopForDraftRecovery(adapter, routeDraft);
      return adapter;
    } else if (adapter.draftSession?.token) {
      // The server rendered the saved page, but this tab retains a draft. Do not
      // silently post that hidden draft or overwrite its only recovery pointer.
      __flyStopForDraftRecovery(adapter, adapter.draftSession.token);
      return adapter;
    } else {
      adapter.draftSession = null;
    }
  } else if (routeDraft && adapter.draftSession?.token !== routeDraft) {
    adapter.draftSession = { token: routeDraft, generation: null };
  }
  const __flyRouteOwnership = () => {
    let currentUrl;
    try { currentUrl = new URL(globalThis.location.href); } catch (_) { currentUrl = null; }
    const samePage = !!(boundUrl && currentUrl &&
      boundUrl.origin === currentUrl.origin && boundUrl.pathname === currentUrl.pathname &&
      boundUrl.searchParams.get("page_id") === currentUrl.searchParams.get("page_id"));
    const connected = adapter.root.isConnected !== false;
    return { samePage, connected, valid: samePage && connected && __flyDraftFromRoute() === boundDraft };
  };
  if (strictDraft) {
    const emitIntent = adapter.emitIntent;
    adapter.emitIntent = function (...args) {
      const ownership = __flyRouteOwnership();
      if (!ownership.valid) {
        __flyStopForDraftRecovery(adapter, adapter.draftSession?.token,
          ownership.samePage && ownership.connected);
        return Promise.resolve(null);
      }
      return emitIntent.apply(adapter, args);
    };
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
    if (!detail?.ok || detail.current === false || !__flyObject(detail.result)) return;
    const token = detail.result.draft_token;
    if (strictDraft) {
      const ownership = __flyRouteOwnership();
      if (!ownership.valid) {
        detail.result.reload = false;
        delete detail.result.location;
        // The adapter still stores an accepted response under its own page key.
        // Do not put a recovery link for the old page on a different route.
        __flyStopForDraftRecovery(adapter, token, ownership.samePage && ownership.connected);
        return;
      }
    }
    if (typeof token === "string" && token && !__flyWriteDraftRoute(token)) {
      // The adapter would otherwise reload a URL that cannot restore this
      // response's session. Keep the response and expose a recovery link.
      detail.result.reload = false;
      // A response may also carry a navigation target; suppress it as well
      // until the user can restore the draft whose route could not be written.
      delete detail.result.location;
      __flyStopForDraftRecovery(adapter, token);
    } else if (typeof token === "string" && token) {
      boundDraft = token;
    }
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
        let end = start
            + source[start..]
                .find("adapter.root.addEventListener")
                .expect("binding end");
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
