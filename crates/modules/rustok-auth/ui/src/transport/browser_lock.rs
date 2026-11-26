//! Origin-wide browser cookie mutation fence. Web Locks is a secure-context
//! browser primitive, not a localStorage lease or an authorization boundary.

use std::{cell::RefCell, future::Future, rc::Rc};

use leptos::wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, future_to_promise};

use crate::model::AuthError;

const COOKIE_LOCK: &str = "rustok:auth:direct-session";

pub(super) fn timeout_signal(milliseconds: u32) -> Result<web_sys::AbortSignal, AuthError> {
    // Use fallible reflection so an older browser lacking AbortSignal.timeout
    // refuses before I/O instead of throwing out of a WASM binding.
    let global = js_sys::global();
    let constructor = js_sys::Reflect::get(&global, &JsValue::from_str("AbortSignal"))
        .map_err(|_| AuthError::Network)?;
    let timeout = js_sys::Reflect::get(&constructor, &JsValue::from_str("timeout"))
        .map_err(|_| AuthError::Network)?
        .dyn_into::<js_sys::Function>().map_err(|_| AuthError::Network)?;
    timeout.call1(&constructor, &JsValue::from_f64(f64::from(milliseconds)))
        .map_err(|_| AuthError::Network)?
        .dyn_into::<web_sys::AbortSignal>().map_err(|_| AuthError::Network)
}

pub(super) async fn with_cookie_lock<T: 'static>(
    operation: impl Future<Output = Result<T, AuthError>> + 'static,
) -> Result<T, AuthError> {
    let window = web_sys::window().ok_or(AuthError::Network)?;
    let navigator = js_sys::Reflect::get(window.as_ref(), &JsValue::from_str("navigator"))
        .map_err(|_| AuthError::Network)?;
    let locks = js_sys::Reflect::get(&navigator, &JsValue::from_str("locks"))
        .map_err(|_| AuthError::Network)?;
    let request = js_sys::Reflect::get(&locks, &JsValue::from_str("request"))
        .map_err(|_| AuthError::Network)?
        .dyn_into::<js_sys::Function>().map_err(|_| AuthError::Network)?;

    let options = js_sys::Object::new();
    let signal = timeout_signal(25_000)?;
    for (name, value) in [
        ("mode", JsValue::from_str("exclusive")),
        ("signal", JsValue::from(signal)),
    ] {
        if !js_sys::Reflect::set(options.as_ref(), &JsValue::from_str(name), &value)
            .map_err(|_| AuthError::Network)?
        {
            return Err(AuthError::Network);
        }
    }

    let result = Rc::new(RefCell::new(None));
    let slot = Rc::clone(&result);
    // The callback's returned Promise keeps the browser lock until all network
    // attempts and response parsing finish, even if the outer Rust future drops.
    // A dropped outer future before lock admission drops the callback; it cannot
    // subsequently dispatch the operation. No credentials enter the JS result.
    let callback = Closure::once_assert_unwind_safe(move |lock: JsValue| -> js_sys::Promise {
        future_to_promise(async move {
            let outcome = if lock.is_null() || lock.is_undefined() {
                Err(AuthError::Http(409))
            } else {
                operation.await
            };
            *slot.borrow_mut() = Some(outcome);
            Ok(JsValue::UNDEFINED)
        })
    });
    let promise = request.call3(
        &locks,
        &JsValue::from_str(COOKIE_LOCK),
        options.as_ref(),
        callback.as_ref(),
    ).map_err(|_| AuthError::Network)?
        .dyn_into::<js_sys::Promise>().map_err(|_| AuthError::Network)?;
    JsFuture::from(promise).await.map_err(|_| AuthError::Network)?;
    drop(callback);
    let outcome = result.borrow_mut().take().ok_or(AuthError::Network)?;
    outcome
}
