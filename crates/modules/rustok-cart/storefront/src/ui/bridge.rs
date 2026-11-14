//! Browser event adapter for the Cart-owned controller. No event contains credentials or cart IDs.

use leptos::prelude::*;

use super::buyer::BuyerCartController;

#[derive(Clone, Copy)]
pub(super) struct DrawerVisibility {
    pub open: RwSignal<bool>,
    #[cfg(target_arch = "wasm32")]
    focus_origin: StoredValue<Option<web_sys::HtmlElement>, leptos::reactive::owner::LocalStorage>,
}

impl DrawerVisibility {
    pub fn new() -> Self {
        Self {
            open: RwSignal::new(false),
            #[cfg(target_arch = "wasm32")]
            focus_origin: StoredValue::new_local(None),
        }
    }

    pub fn set(self, next: bool) -> bool {
        let Some(previous) = self.open.try_get_untracked() else {
            return false;
        };
        if previous == next {
            return true;
        }
        #[cfg(target_arch = "wasm32")]
        if next {
            use leptos::wasm_bindgen::JsCast;
            let active = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.active_element())
                .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok());
            if self.focus_origin.try_set_value(active).is_some() {
                return false;
            }
        }
        if self.open.try_update(|open| *open = next).is_none() {
            return false;
        }
        #[cfg(target_arch = "wasm32")]
        if !next {
            let target = self
                .focus_origin
                .try_update_value(|value| value.take())
                .flatten();
            if let Some(target) = target.filter(|target| target.is_connected()) {
                let _ = target.focus();
            }
        }
        true
    }

    pub fn toggle(self) -> bool {
        self.open
            .try_get_untracked()
            .is_some_and(|previous| self.set(!previous))
    }
}

/// Invalidate synchronously at the start of a transition; observations are published by the
/// mounted owner after a checked read/write, never inferred from browser storage.
pub(crate) fn publish_count(count: Option<u64>) {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::JsValue;
        let Some(window) = web_sys::window() else {
            return;
        };
        let detail = js_sys::Object::new();
        let (key, value) = match count {
            Some(count) if count <= 9_007_199_254_740_991 => {
                ("count", JsValue::from_f64(count as f64))
            }
            _ => ("status", JsValue::from_str("unknown")),
        };
        if js_sys::Reflect::set(&detail, &JsValue::from_str(key), &value).is_err() {
            return;
        }
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&detail);
        if let Ok(event) =
            web_sys::CustomEvent::new_with_event_init_dict("rustok:cart:updated", &init)
        {
            let _ = window.dispatch_event(&event);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = count;
}

#[cfg(target_arch = "wasm32")]
struct Listener {
    window: web_sys::Window,
    name: &'static str,
    callback: leptos::wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>,
}

#[cfg(target_arch = "wasm32")]
impl Listener {
    fn new(
        window: &web_sys::Window,
        name: &'static str,
        handler: impl FnMut(web_sys::Event) + 'static,
    ) -> Result<Self, leptos::wasm_bindgen::JsValue> {
        use leptos::wasm_bindgen::JsCast;
        let callback = leptos::wasm_bindgen::closure::Closure::wrap(
            Box::new(handler) as Box<dyn FnMut(web_sys::Event)>
        );
        window.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
        Ok(Self {
            window: window.clone(),
            name,
            callback,
        })
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for Listener {
    fn drop(&mut self) {
        use leptos::wasm_bindgen::JsCast;
        let _ = self
            .window
            .remove_event_listener_with_callback(self.name, self.callback.as_ref().unchecked_ref());
    }
}

/// Installs exactly one owner consumer for this mount and removes every listener on cleanup.
pub(super) fn install(
    controller: BuyerCartController,
    drawer: DrawerVisibility,
    auth_source: String,
) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::JsCast;
        let Some(window) = web_sys::window() else {
            return false;
        };
        let mut listeners = Vec::new();
        let result: Result<(), leptos::wasm_bindgen::JsValue> = (|| {
            for name in [
                "rustok:cart:open",
                "rustok:cart:close",
                "rustok:cart:toggle",
            ] {
                listeners.push(Listener::new(&window, name, move |event| {
                    if !event.cancelable()
                        || event.default_prevented()
                        || !controller.context_ready_untracked()
                    {
                        return;
                    }
                    let handled = match name {
                        "rustok:cart:open" => drawer.set(true),
                        "rustok:cart:close" => drawer.set(false),
                        "rustok:cart:toggle" => drawer.toggle(),
                        _ => false,
                    };
                    // A synchronous focus/event callback can dispose or invalidate the owner.
                    // Acknowledge only if it is still current after accepting the request.
                    if handled && controller.context_ready_untracked() {
                        event.prevent_default();
                    }
                })?);
            }
            listeners.push(Listener::new(
                &window,
                "rustok:cart:request-count",
                move |_| {
                    publish_count(controller.confirmed_item_count_untracked());
                },
            )?);
            listeners.push(Listener::new(
                &window,
                rustok_auth_ui::ui::leptos::AUTH_SCOPE_CHANGE_EVENT,
                move |event| {
                    publish_count(None);
                    let detail = event
                        .dyn_ref::<web_sys::CustomEvent>()
                        .and_then(|event| event.detail().as_string());
                    match detail.as_deref().and_then(|value| value.rsplit_once(':')) {
                        Some((source, _)) if source != auth_source => {
                            controller.invalidate_external_auth();
                            drawer.set(false);
                        }
                        Some(_) => {}
                        None => {
                            controller.invalidate_external_auth();
                            drawer.set(false);
                        }
                    }
                },
            )?);
            for name in [
                "popstate",
                "rustok:storefront:navigation",
                "rustok:storefront:tenant-changed",
            ] {
                listeners.push(Listener::new(&window, name, move |_| {
                    publish_count(None);
                    controller.invalidate_external_auth();
                    drawer.set(false);
                })?);
            }
            listeners.push(Listener::new(&window, "storage", move |event| {
                let key = event
                    .dyn_ref::<web_sys::StorageEvent>()
                    .and_then(|event| event.key());
                if key.as_deref().is_none_or(|key| {
                    key == rustok_auth_ui::ui::leptos::AUTH_SCOPE_CHANGE_STORAGE_KEY
                        || key == rustok_auth_ui::core::ADMIN_TENANT_KEY
                        || controller.matches_checkpoint_key(key)
                }) {
                    publish_count(None);
                    controller.invalidate_external_auth();
                    drawer.set(false);
                }
            })?);
            Ok(())
        })();
        if result.is_err() {
            // Partial registration is removed by Drop. No control is acknowledged.
            return false;
        }
        let listeners = StoredValue::new_local(Some(listeners));
        on_cleanup(move || {
            // Detach consumers before publishing the final unknown observation. Publishing is
            // synchronous and can itself trigger another owner cleanup.
            drop(listeners.try_update_value(Option::take));
            publish_count(None);
        });
        Effect::new(move |_| {
            if !controller.context_ready() {
                drawer.set(false);
            }
            publish_count(controller.confirmed_item_count());
        });
        true
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (controller, drawer, auth_source);
        false
    }
}
