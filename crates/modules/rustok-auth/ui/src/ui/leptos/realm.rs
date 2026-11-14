//! Same-document coordination for the shared browser auth cookie.
//!
//! Separate Leptos roots (and separate WASM instances) use the same window slot. The slot is
//! only an in-realm consistency fence: it does not authenticate a request, coordinate tabs,
//! cancel an already-sent request or constrain unrelated scripts that ignore the protocol.

use crate::model::AuthError;

#[cfg(target_arch = "wasm32")]
const REALM_TRANSITION_KEY: &str = "__rustokAuthTransition";

pub(crate) struct RealmTransition {
    #[cfg(target_arch = "wasm32")]
    stamp: String,
    released: bool,
}

impl RealmTransition {
    pub(crate) fn acquire(source: &str) -> Result<Self, AuthError> {
        #[cfg(target_arch = "wasm32")]
        {
            use leptos::wasm_bindgen::JsValue;
            let window = web_sys::window().ok_or(AuthError::Network)?;
            let key = JsValue::from_str(REALM_TRANSITION_KEY);
            let state =
                js_sys::Reflect::get(window.as_ref(), &key).map_err(|_| AuthError::Network)?;
            if !state.is_undefined() && state.as_string().as_deref() != Some("idle") {
                // A pending, abandoned or malformed slot must not start another cookie operation.
                return Err(AuthError::Http(409));
            }
            let stamp = format!("pending:{source}");
            let value = JsValue::from_str(&stamp);
            if !js_sys::Reflect::set(window.as_ref(), &key, &value).unwrap_or(false)
                || js_sys::Reflect::get(window.as_ref(), &key).ok().as_ref() != Some(&value)
            {
                // No request was sent. A malformed slot must not be treated as an idle lease.
                let _ = js_sys::Reflect::set(window.as_ref(), &key, &JsValue::from_str("poisoned"));
                return Err(AuthError::Network);
            }
            Ok(Self {
                stamp,
                released: false,
            })
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = source;
            // Native SSR requests have distinct request owners, not a shared browser cookie.
            Ok(Self { released: false })
        }
    }

    pub(crate) fn is_current(&self) -> bool {
        if self.released {
            return false;
        }
        #[cfg(target_arch = "wasm32")]
        {
            use leptos::wasm_bindgen::JsValue;
            web_sys::window()
                .and_then(|window| {
                    js_sys::Reflect::get(window.as_ref(), &JsValue::from_str(REALM_TRANSITION_KEY))
                        .ok()
                })
                .and_then(|state| state.as_string())
                .as_deref()
                == Some(self.stamp.as_str())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            true
        }
    }

    /// Release only a completed (or definitely unsent) operation's own lease.
    pub(crate) fn release(mut self) -> bool {
        #[cfg(target_arch = "wasm32")]
        let released = {
            use leptos::wasm_bindgen::JsValue;
            let key = JsValue::from_str(REALM_TRANSITION_KEY);
            let idle = JsValue::from_str("idle");
            self.is_current()
                && web_sys::window().is_some_and(|window| {
                    js_sys::Reflect::set(window.as_ref(), &key, &idle).unwrap_or(false)
                        && js_sys::Reflect::get(window.as_ref(), &key).ok().as_ref() == Some(&idle)
                })
        };
        #[cfg(not(target_arch = "wasm32"))]
        let released = self.is_current();
        if !released {
            self.poison();
        }
        self.released = true;
        released
    }

    /// An abandoned future may still receive Set-Cookie. Never re-open this document's slot.
    pub(crate) fn poison(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            use leptos::wasm_bindgen::JsValue;
            if let Some(window) = web_sys::window() {
                let _ = js_sys::Reflect::set(
                    window.as_ref(),
                    &JsValue::from_str(REALM_TRANSITION_KEY),
                    &JsValue::from_str("poisoned"),
                );
            }
        }
        self.released = true;
    }
}

impl Drop for RealmTransition {
    fn drop(&mut self) {
        if !self.released {
            self.poison();
        }
    }
}
