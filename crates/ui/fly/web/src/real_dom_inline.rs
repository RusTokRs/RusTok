use fly::ProjectHash;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};

pub const FLY_REAL_DOM_COMPONENT_ATTRIBUTE: &str = "data-fly-component-id";
pub const FLY_REAL_DOM_INLINE_ATTRIBUTE: &str = "data-fly-inline-editable";
pub const MAX_INLINE_TEXT_BYTES: usize = 64 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct AuthenticatedInlineEditGrant {
    session_id: String,
    page_id: String,
    revision_id: String,
    expected_project_hash: ProjectHash,
    authorization_proof: String,
    expires_at_unix_ms: u64,
}

impl AuthenticatedInlineEditGrant {
    pub fn new(
        session_id: impl Into<String>,
        page_id: impl Into<String>,
        revision_id: impl Into<String>,
        expected_project_hash: ProjectHash,
        authorization_proof: impl Into<String>,
        expires_at_unix_ms: u64,
    ) -> Result<Self, InlineEditContractError> {
        let grant = Self {
            session_id: normalized_required(session_id.into(), "session id")?,
            page_id: normalized_required(page_id.into(), "page id")?,
            revision_id: normalized_required(revision_id.into(), "revision id")?,
            expected_project_hash,
            authorization_proof: normalized_required(
                authorization_proof.into(),
                "authorization proof",
            )?,
            expires_at_unix_ms,
        };
        if grant.expires_at_unix_ms == 0 {
            return Err(InlineEditContractError::InvalidGrant(
                "expiry must be a positive Unix timestamp".to_string(),
            ));
        }
        Ok(grant)
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn page_id(&self) -> &str {
        &self.page_id
    }

    pub fn revision_id(&self) -> &str {
        &self.revision_id
    }

    pub const fn expected_project_hash(&self) -> ProjectHash {
        self.expected_project_hash
    }

    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }

    pub fn is_expired(&self, now_unix_ms: u64) -> bool {
        now_unix_ms >= self.expires_at_unix_ms
    }

    pub fn bind_request(
        &self,
        now_unix_ms: u64,
        sequence: u64,
        component_id: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<AuthenticatedInlineEditRequest, InlineEditContractError> {
        if self.is_expired(now_unix_ms) {
            return Err(InlineEditContractError::ExpiredGrant);
        }
        if sequence == 0 {
            return Err(InlineEditContractError::InvalidSequence);
        }
        let component_id = normalized_required(component_id.into(), "component id")?;
        let value = normalize_plain_text(value.into())?;
        Ok(AuthenticatedInlineEditRequest {
            session_id: self.session_id.clone(),
            page_id: self.page_id.clone(),
            revision_id: self.revision_id.clone(),
            expected_project_hash: self.expected_project_hash,
            authorization_proof: self.authorization_proof.clone(),
            expires_at_unix_ms: self.expires_at_unix_ms,
            sequence,
            component_id,
            field: InlineEditableField::Content,
            value,
        })
    }

    pub fn validate_request(
        &self,
        request: &AuthenticatedInlineEditRequest,
        now_unix_ms: u64,
    ) -> Result<(), InlineEditContractError> {
        if self.is_expired(now_unix_ms) || request.expires_at_unix_ms <= now_unix_ms {
            return Err(InlineEditContractError::ExpiredGrant);
        }
        if request.session_id != self.session_id
            || request.page_id != self.page_id
            || request.revision_id != self.revision_id
            || request.expected_project_hash != self.expected_project_hash
            || request.authorization_proof != self.authorization_proof
            || request.expires_at_unix_ms != self.expires_at_unix_ms
        {
            return Err(InlineEditContractError::GrantIdentityMismatch);
        }
        if request.sequence == 0 {
            return Err(InlineEditContractError::InvalidSequence);
        }
        normalize_plain_text(request.value.clone())?;
        Ok(())
    }
}

impl Debug for AuthenticatedInlineEditGrant {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthenticatedInlineEditGrant")
            .field("session_id", &self.session_id)
            .field("page_id", &self.page_id)
            .field("revision_id", &self.revision_id)
            .field("expected_project_hash", &self.expected_project_hash)
            .field("authorization_proof", &"[REDACTED]")
            .field("expires_at_unix_ms", &self.expires_at_unix_ms)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InlineEditableField {
    Content,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthenticatedInlineEditRequest {
    pub session_id: String,
    pub page_id: String,
    pub revision_id: String,
    pub expected_project_hash: ProjectHash,
    authorization_proof: String,
    pub expires_at_unix_ms: u64,
    pub sequence: u64,
    pub component_id: String,
    pub field: InlineEditableField,
    pub value: String,
}

impl AuthenticatedInlineEditRequest {
    pub fn authorization_proof(&self) -> &str {
        &self.authorization_proof
    }
}

impl Debug for AuthenticatedInlineEditRequest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthenticatedInlineEditRequest")
            .field("session_id", &self.session_id)
            .field("page_id", &self.page_id)
            .field("revision_id", &self.revision_id)
            .field("expected_project_hash", &self.expected_project_hash)
            .field("authorization_proof", &"[REDACTED]")
            .field("expires_at_unix_ms", &self.expires_at_unix_ms)
            .field("sequence", &self.sequence)
            .field("component_id", &self.component_id)
            .field("field", &self.field)
            .field("value", &self.value)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineEditContractError {
    InvalidGrant(String),
    ExpiredGrant,
    GrantIdentityMismatch,
    InvalidSequence,
    InvalidPlainText(String),
    Browser(String),
}

impl Display for InlineEditContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGrant(message) => {
                write!(formatter, "invalid inline edit grant: {message}")
            }
            Self::ExpiredGrant => formatter.write_str("inline edit grant has expired"),
            Self::GrantIdentityMismatch => {
                formatter.write_str("inline edit request does not match its trusted grant")
            }
            Self::InvalidSequence => {
                formatter.write_str("inline edit sequence must be greater than zero")
            }
            Self::InvalidPlainText(message) => {
                write!(formatter, "invalid inline edit plain text: {message}")
            }
            Self::Browser(message) => {
                write!(formatter, "inline edit browser adapter failed: {message}")
            }
        }
    }
}

impl std::error::Error for InlineEditContractError {}

fn normalized_required(value: String, label: &str) -> Result<String, InlineEditContractError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(InlineEditContractError::InvalidGrant(format!(
            "{label} must not be empty"
        )))
    } else {
        Ok(value)
    }
}

fn normalize_plain_text(value: String) -> Result<String, InlineEditContractError> {
    let value = value.replace("\r\n", "\n").replace('\r', "\n");
    if value.contains('\0') {
        return Err(InlineEditContractError::InvalidPlainText(
            "NUL bytes are not allowed".to_string(),
        ));
    }
    if value.len() > MAX_INLINE_TEXT_BYTES {
        return Err(InlineEditContractError::InvalidPlainText(format!(
            "value is {} bytes; maximum is {MAX_INLINE_TEXT_BYTES}",
            value.len()
        )));
    }
    Ok(value)
}

#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
mod browser {
    use super::*;
    use std::cell::Cell;
    use std::collections::BTreeSet;
    use std::rc::Rc;
    use wasm_bindgen::{JsCast, closure::Closure};
    use web_sys::{Element, Event, HtmlElement};

    struct MarkedElement {
        element: Element,
        contenteditable: Option<String>,
        inline_marker: Option<String>,
        role: Option<String>,
        tabindex: Option<String>,
        spellcheck: Option<String>,
        restored: bool,
    }

    impl MarkedElement {
        fn mark(element: Element) -> Result<Self, InlineEditContractError> {
            let mut snapshot = Self {
                contenteditable: element.get_attribute("contenteditable"),
                inline_marker: element.get_attribute(FLY_REAL_DOM_INLINE_ATTRIBUTE),
                role: element.get_attribute("role"),
                tabindex: element.get_attribute("tabindex"),
                spellcheck: element.get_attribute("spellcheck"),
                element: element.clone(),
                restored: false,
            };
            let result = (|| {
                element
                    .set_attribute("contenteditable", "plaintext-only")
                    .map_err(browser_error)?;
                element
                    .set_attribute(FLY_REAL_DOM_INLINE_ATTRIBUTE, "content")
                    .map_err(browser_error)?;
                if snapshot.role.is_none() {
                    element
                        .set_attribute("role", "textbox")
                        .map_err(browser_error)?;
                }
                if snapshot.tabindex.is_none() {
                    element
                        .set_attribute("tabindex", "0")
                        .map_err(browser_error)?;
                }
                if snapshot.spellcheck.is_none() {
                    element
                        .set_attribute("spellcheck", "true")
                        .map_err(browser_error)?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                snapshot.restore();
                return Err(error);
            }
            Ok(snapshot)
        }

        fn restore(&mut self) {
            if self.restored {
                return;
            }
            restore_attribute(
                &self.element,
                "contenteditable",
                self.contenteditable.take(),
            );
            restore_attribute(
                &self.element,
                FLY_REAL_DOM_INLINE_ATTRIBUTE,
                self.inline_marker.take(),
            );
            restore_attribute(&self.element, "role", self.role.take());
            restore_attribute(&self.element, "tabindex", self.tabindex.take());
            restore_attribute(&self.element, "spellcheck", self.spellcheck.take());
            self.restored = true;
        }
    }

    impl Drop for MarkedElement {
        fn drop(&mut self) {
            self.restore();
        }
    }

    pub struct RealDomInlineEditSubscription {
        root: Element,
        focusout: Closure<dyn FnMut(Event)>,
        marked: Vec<MarkedElement>,
    }

    impl Drop for RealDomInlineEditSubscription {
        fn drop(&mut self) {
            let _ = self.root.remove_event_listener_with_callback(
                "focusout",
                self.focusout.as_ref().unchecked_ref(),
            );
            self.marked.clear();
        }
    }

    pub fn attach_real_dom_inline_editing(
        root_id: &str,
        editable_component_ids: impl IntoIterator<Item = String>,
        grant: AuthenticatedInlineEditGrant,
        on_request: impl Fn(AuthenticatedInlineEditRequest) + 'static,
        on_error: impl Fn(InlineEditContractError) + 'static,
    ) -> Result<RealDomInlineEditSubscription, InlineEditContractError> {
        let window = web_sys::window()
            .ok_or_else(|| InlineEditContractError::Browser("window is unavailable".to_string()))?;
        let document = window.document().ok_or_else(|| {
            InlineEditContractError::Browser("document is unavailable".to_string())
        })?;
        let root = document.get_element_by_id(root_id).ok_or_else(|| {
            InlineEditContractError::Browser(format!("root element `{root_id}` was not found"))
        })?;
        let allowed = editable_component_ids.into_iter().collect::<BTreeSet<_>>();
        let nodes = root
            .query_selector_all(&format!("[{FLY_REAL_DOM_COMPONENT_ATTRIBUTE}]"))
            .map_err(browser_error)?;
        let mut marked = Vec::new();
        for index in 0..nodes.length() {
            let Some(node) = nodes.item(index) else {
                continue;
            };
            let Ok(element) = node.dyn_into::<Element>() else {
                continue;
            };
            let Some(component_id) = element.get_attribute(FLY_REAL_DOM_COMPONENT_ATTRIBUTE) else {
                continue;
            };
            if allowed.contains(&component_id) {
                marked.push(MarkedElement::mark(element)?);
            }
        }

        let allowed = Rc::new(allowed);
        let sequence = Rc::new(Cell::new(0_u64));
        let on_request = Rc::new(on_request);
        let on_error = Rc::new(on_error);
        let focusout = Closure::<dyn FnMut(Event)>::new({
            let allowed = allowed.clone();
            let sequence = sequence.clone();
            let on_request = on_request.clone();
            let on_error = on_error.clone();
            move |event: Event| {
                let Some(target) = event
                    .target()
                    .and_then(|target| target.dyn_into::<Element>().ok())
                else {
                    return;
                };
                let editable = target
                    .closest(&format!(
                        "[{FLY_REAL_DOM_INLINE_ATTRIBUTE}='content'][{FLY_REAL_DOM_COMPONENT_ATTRIBUTE}]"
                    ))
                    .ok()
                    .flatten();
                let Some(editable) = editable else {
                    return;
                };
                let Some(component_id) = editable.get_attribute(FLY_REAL_DOM_COMPONENT_ATTRIBUTE)
                else {
                    return;
                };
                if !allowed.contains(&component_id) {
                    return;
                }
                let Some(element) = editable.dyn_ref::<HtmlElement>() else {
                    return;
                };
                let next_sequence = sequence.get().saturating_add(1);
                let now_unix_ms = js_sys::Date::now().max(0.0) as u64;
                match grant.bind_request(
                    now_unix_ms,
                    next_sequence,
                    component_id,
                    element.inner_text(),
                ) {
                    Ok(request) => {
                        sequence.set(next_sequence);
                        on_request(request);
                    }
                    Err(error) => on_error(error),
                }
            }
        });
        if let Err(error) =
            root.add_event_listener_with_callback("focusout", focusout.as_ref().unchecked_ref())
        {
            marked.clear();
            return Err(browser_error(error));
        }

        Ok(RealDomInlineEditSubscription {
            root,
            focusout,
            marked,
        })
    }

    fn restore_attribute(element: &Element, name: &str, value: Option<String>) {
        match value {
            Some(value) => {
                let _ = element.set_attribute(name, &value);
            }
            None => {
                let _ = element.remove_attribute(name);
            }
        }
    }

    fn browser_error(value: wasm_bindgen::JsValue) -> InlineEditContractError {
        InlineEditContractError::Browser(
            value
                .as_string()
                .unwrap_or_else(|| "unknown browser error".to_string()),
        )
    }
}

#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
pub use browser::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn grant() -> AuthenticatedInlineEditGrant {
        AuthenticatedInlineEditGrant::new(
            "session-a",
            "page-a",
            "revision-a",
            ProjectHash(7),
            "signed-proof",
            2_000,
        )
        .expect("grant")
    }

    #[test]
    fn grant_binds_identity_and_normalizes_plain_text() {
        let request = grant()
            .bind_request(1_000, 1, "hero", "Hello\r\nworld")
            .expect("request");
        assert_eq!(request.value, "Hello\nworld");
        assert_eq!(request.expected_project_hash, ProjectHash(7));
        assert_eq!(request.authorization_proof(), "signed-proof");
        assert!(!format!("{request:?}").contains("signed-proof"));
        grant()
            .validate_request(&request, 1_000)
            .expect("same grant must validate request");
    }

    #[test]
    fn grant_fails_closed_on_expiry_and_identity_drift() {
        assert_eq!(
            grant().bind_request(2_000, 1, "hero", "Hello"),
            Err(InlineEditContractError::ExpiredGrant)
        );
        let mut request = grant()
            .bind_request(1_000, 1, "hero", "Hello")
            .expect("request");
        request.revision_id = "other".to_string();
        assert_eq!(
            grant().validate_request(&request, 1_000),
            Err(InlineEditContractError::GrantIdentityMismatch)
        );
    }

    #[test]
    fn request_rejects_nul_and_oversized_plain_text() {
        assert!(matches!(
            grant().bind_request(1_000, 1, "hero", "bad\0value"),
            Err(InlineEditContractError::InvalidPlainText(_))
        ));
        assert!(matches!(
            grant().bind_request(1_000, 1, "hero", "x".repeat(MAX_INLINE_TEXT_BYTES + 1)),
            Err(InlineEditContractError::InvalidPlainText(_))
        ));
    }
}
