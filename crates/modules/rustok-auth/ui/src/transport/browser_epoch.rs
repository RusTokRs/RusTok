//! One non-secret, origin-wide client transition epoch. It fences local
//! projections; only the backend decides credential validity or authorization.

use crate::model::AuthError;

pub const AUTH_SCOPE_CHANGE_EVENT: &str = "rustok:auth:scope-change";
pub const AUTH_SCOPE_CHANGE_STORAGE_KEY: &str = "rustok-auth-scope-change";

pub(crate) type Epoch = Option<String>;

#[cfg(target_arch = "wasm32")]
pub(crate) fn read() -> Result<Epoch, AuthError> {
    let window = web_sys::window().ok_or(AuthError::Network)?;
    let storage = window
        .local_storage()
        .map_err(|_| AuthError::Network)?
        .ok_or(AuthError::Network)?;
    storage
        .get_item(AUTH_SCOPE_CHANGE_STORAGE_KEY)
        .map_err(|_| AuthError::Network)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn read() -> Result<Epoch, AuthError> {
    Ok(None)
}

pub(crate) fn matches(expected: &Epoch) -> bool {
    epoch_matches(read(), expected)
}

fn epoch_matches(observed: Result<Epoch, AuthError>, expected: &Epoch) -> bool {
    observed.is_ok_and(|current| &current == expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_removed_or_unreadable_epoch_refuses_queued_intent() {
        let expected = Some("source:original".to_owned());
        assert!(epoch_matches(Ok(expected.clone()), &expected));
        assert!(!epoch_matches(Ok(Some("other:next".to_owned())), &expected));
        assert!(!epoch_matches(Ok(None), &expected));
        assert!(!epoch_matches(Err(AuthError::Network), &expected));
        assert!(epoch_matches(Ok(None), &None));
        assert!(!epoch_matches(Err(AuthError::Network), &None));
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn publish(source: &str, identity_change: bool) -> Result<Epoch, AuthError> {
    use leptos::wasm_bindgen::JsValue;
    let window = web_sys::window().ok_or(AuthError::Network)?;
    let storage = window
        .local_storage()
        .map_err(|_| AuthError::Network)?
        .ok_or(AuthError::Network)?;
    let mut nonce = [0u8; 16];
    window
        .crypto()
        .map_err(|_| AuthError::Network)?
        .get_random_values_with_u8_array(&mut nonce)
        .map_err(|_| AuthError::Network)?;
    let nonce = nonce
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("");
    let stamp = format!("{source}:{nonce}");
    storage
        .set_item(AUTH_SCOPE_CHANGE_STORAGE_KEY, &stamp)
        .map_err(|_| AuthError::Network)?;
    let expected = Some(stamp);
    if !matches(&expected) {
        return Err(AuthError::Http(409));
    }
    // Preserve the existing same-document notification contract for Cart/Fly.
    let detail = format!(
        "{source}:{}",
        if identity_change {
            "identity"
        } else {
            "refresh"
        }
    );
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&JsValue::from_str(&detail));
    let event = web_sys::CustomEvent::new_with_event_init_dict(AUTH_SCOPE_CHANGE_EVENT, &init)
        .map_err(|_| AuthError::Network)?;
    window
        .dispatch_event(&event)
        .map_err(|_| AuthError::Network)?;
    if !matches(&expected) {
        return Err(AuthError::Http(409));
    }
    Ok(expected)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn publish(_source: &str, _identity_change: bool) -> Result<Epoch, AuthError> {
    Ok(None)
}
