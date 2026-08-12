use fly::{
    ComponentNode, ComponentObject, ComponentPatch, EditorCommand, FlyEditor, GrapesJsCodec,
    PageSelection, ProjectDocument, ProjectHash, RegistrySet, RenderPolicy, RenderedPage,
};
use fly_leptos::{
    AuthenticatedInlineEditGrant, AuthenticatedInlineEditRequest, InlineEditContractError,
    InlineEditableField,
};
use leptos::prelude::*;
use rustok_page_builder::render::{
    PageBuilderRenderRequest, PageBuilderRenderResponse, PageBuilderRuntimeRenderRequest,
    PageBuilderRuntimeRenderResponse, render_page_builder_project, render_page_builder_runtime,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};

pub use fly_leptos::{
    AuthenticatedInlineEditGrant as InlineEditGrant,
    AuthenticatedInlineEditRequest as InlineEditRequest,
};

const RUNTIME_COMPONENT_COLLECTIONS: [&str; 3] = [
    "flyRuntimeBindings",
    "flyRuntimeConditions",
    "flyRuntimeRepeaters",
];

pub trait InlineEditAuthorizationPort {
    fn authorize(&self, request: &AuthenticatedInlineEditRequest) -> Result<(), String>;
}

impl<F> InlineEditAuthorizationPort for F
where
    F: Fn(&AuthenticatedInlineEditRequest) -> Result<(), String>,
{
    fn authorize(&self, request: &AuthenticatedInlineEditRequest) -> Result<(), String> {
        self(request)
    }
}

pub struct AuthenticatedInlineEditSession {
    grant: AuthenticatedInlineEditGrant,
    selection: PageSelection,
    editor: FlyEditor,
    last_sequence: u64,
}

impl AuthenticatedInlineEditSession {
    pub fn new(
        project_data: Value,
        selection: PageSelection,
        grant: AuthenticatedInlineEditGrant,
        now_unix_ms: u64,
    ) -> Result<Self, InlineEditError> {
        if grant.is_expired(now_unix_ms) {
            return Err(InlineEditError::Contract(
                InlineEditContractError::ExpiredGrant,
            ));
        }
        let document = GrapesJsCodec::decode_value(project_data)?;
        let page_index = selected_page_index(&document, &selection)?;
        let page_id = document.project.pages[page_index]
            .id
            .as_deref()
            .ok_or(InlineEditError::SelectedPageMissingId)?;
        if page_id != grant.page_id() {
            return Err(InlineEditError::GrantPageMismatch {
                expected: page_id.to_string(),
                actual: grant.page_id().to_string(),
            });
        }
        let project_hash = document.hash();
        if project_hash != grant.expected_project_hash() {
            return Err(InlineEditError::StaleProjectHash {
                expected: grant.expected_project_hash(),
                actual: project_hash,
            });
        }
        Ok(Self {
            grant,
            selection,
            editor: FlyEditor::new(document, RegistrySet::with_builtins()),
            last_sequence: 0,
        })
    }

    pub fn editable_component_ids(&self) -> Vec<String> {
        editable_component_ids_from_document(self.editor.document(), &self.selection)
            .unwrap_or_default()
    }

    pub fn current_project_hash(&self) -> ProjectHash {
        self.editor.revision().project_hash
    }

    pub fn apply_authorized(
        &mut self,
        request: AuthenticatedInlineEditRequest,
        now_unix_ms: u64,
        authorization: &dyn InlineEditAuthorizationPort,
    ) -> Result<InlineEditApplyResult, InlineEditError> {
        self.grant.validate_request(&request, now_unix_ms)?;
        if request.sequence <= self.last_sequence {
            return Err(InlineEditError::SequenceReplay {
                last: self.last_sequence,
                received: request.sequence,
            });
        }
        let current_hash = self.editor.revision().project_hash;
        if request.expected_project_hash != current_hash {
            return Err(InlineEditError::StaleProjectHash {
                expected: request.expected_project_hash,
                actual: current_hash,
            });
        }
        if request.field != InlineEditableField::Content {
            return Err(InlineEditError::UnsupportedField);
        }
        let page_index = selected_page_index(self.editor.document(), &self.selection)?;
        let location = self
            .editor
            .document()
            .component_location(&request.component_id)
            .ok_or_else(|| InlineEditError::ComponentNotFound(request.component_id.clone()))?;
        if location.page_index != page_index {
            return Err(InlineEditError::ComponentOutsideSelectedPage(
                request.component_id.clone(),
            ));
        }
        let runtime_owned = runtime_owned_component_ids(self.editor.document());
        let component = self
            .editor
            .document()
            .component(&request.component_id)
            .ok_or_else(|| InlineEditError::ComponentNotFound(request.component_id.clone()))?;
        if runtime_owned.contains(&request.component_id) || !is_inline_text_component(component) {
            return Err(InlineEditError::ComponentNotInlineEditable(
                request.component_id.clone(),
            ));
        }
        if component.extensions.get("content").and_then(Value::as_str)
            == Some(request.value.as_str())
        {
            return Err(InlineEditError::NoContentChange(
                request.component_id.clone(),
            ));
        }
        authorization
            .authorize(&request)
            .map_err(InlineEditError::AuthorizationRejected)?;

        let previous_hash = current_hash;
        self.editor.apply(EditorCommand::Patch {
            component_id: request.component_id.clone(),
            patch: ComponentPatch::default().set_field("content", json!(request.value.clone())),
        })?;
        self.last_sequence = request.sequence;
        let project_hash = self.editor.revision().project_hash;
        let project_data = GrapesJsCodec::encode_value(self.editor.document())?;
        Ok(InlineEditApplyResult {
            page_id: request.page_id,
            revision_id: request.revision_id,
            component_id: request.component_id,
            sequence: request.sequence,
            previous_hash,
            project_hash,
            command_sequence: self.editor.revision().command_sequence,
            value: request.value,
            project_data,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlineEditApplyResult {
    pub page_id: String,
    pub revision_id: String,
    pub component_id: String,
    pub sequence: u64,
    pub previous_hash: ProjectHash,
    pub project_hash: ProjectHash,
    pub command_sequence: u64,
    pub value: String,
    pub project_data: Value,
}

#[derive(Debug)]
pub enum InlineEditError {
    Contract(InlineEditContractError),
    Fly(fly::FlyError),
    UnsupportedSelection,
    SelectedPageNotFound,
    SelectedPageMissingId,
    GrantPageMismatch {
        expected: String,
        actual: String,
    },
    StaleProjectHash {
        expected: ProjectHash,
        actual: ProjectHash,
    },
    AuthorizationRejected(String),
    SequenceReplay {
        last: u64,
        received: u64,
    },
    UnsupportedField,
    ComponentNotFound(String),
    ComponentOutsideSelectedPage(String),
    ComponentNotInlineEditable(String),
    NoContentChange(String),
}

impl Display for InlineEditError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contract(error) => Display::fmt(error, formatter),
            Self::Fly(error) => Display::fmt(error, formatter),
            Self::UnsupportedSelection => formatter.write_str(
                "slug page selection is not accepted by the authenticated inline edit owner",
            ),
            Self::SelectedPageNotFound => formatter.write_str("selected Fly page was not found"),
            Self::SelectedPageMissingId => {
                formatter.write_str("selected Fly page does not have a stable id")
            }
            Self::GrantPageMismatch { expected, actual } => write!(
                formatter,
                "inline edit grant page `{actual}` does not match selected page `{expected}`"
            ),
            Self::StaleProjectHash { expected, actual } => write!(
                formatter,
                "inline edit project hash is stale: expected {}, actual {}",
                expected.hex(),
                actual.hex()
            ),
            Self::AuthorizationRejected(message) => {
                write!(formatter, "inline edit authorization rejected: {message}")
            }
            Self::SequenceReplay { last, received } => write!(
                formatter,
                "inline edit sequence {received} is not newer than {last}"
            ),
            Self::UnsupportedField => formatter.write_str("inline edit field is unsupported"),
            Self::ComponentNotFound(component_id) => {
                write!(
                    formatter,
                    "inline edit component `{component_id}` was not found"
                )
            }
            Self::ComponentOutsideSelectedPage(component_id) => write!(
                formatter,
                "inline edit component `{component_id}` is outside the selected page"
            ),
            Self::ComponentNotInlineEditable(component_id) => write!(
                formatter,
                "component `{component_id}` is not eligible for plain-text inline editing"
            ),
            Self::NoContentChange(component_id) => write!(
                formatter,
                "inline edit component `{component_id}` did not change"
            ),
        }
    }
}

impl std::error::Error for InlineEditError {}

impl From<InlineEditContractError> for InlineEditError {
    fn from(value: InlineEditContractError) -> Self {
        Self::Contract(value)
    }
}

impl From<fly::FlyError> for InlineEditError {
    fn from(value: fly::FlyError) -> Self {
        Self::Fly(value)
    }
}

pub fn inline_editable_component_ids(
    project_data: &Value,
    selection: &PageSelection,
) -> Result<Vec<String>, InlineEditError> {
    let document = GrapesJsCodec::decode_value(project_data.clone())?;
    editable_component_ids_from_document(&document, selection)
}

pub fn is_inline_text_component(component: &ComponentObject) -> bool {
    let Some(content) = component.extensions.get("content").and_then(Value::as_str) else {
        return false;
    };
    if content.contains("{{") || content.contains("}}") {
        return false;
    }
    if component.provider.is_some() || !component.children().is_empty() {
        return false;
    }
    let component_type = component.component_type().to_ascii_lowercase();
    let tag_name = component
        .tag_name
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(component_type.as_str(), "text" | "heading" | "paragraph")
        || matches!(
            tag_name.as_str(),
            "p" | "span" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        )
}

fn editable_component_ids_from_document(
    document: &ProjectDocument,
    selection: &PageSelection,
) -> Result<Vec<String>, InlineEditError> {
    let page_index = selected_page_index(document, selection)?;
    let runtime_owned = runtime_owned_component_ids(document);
    let mut ids = Vec::new();
    if let Some(root) = document.project.pages[page_index].component.as_ref() {
        collect_editable_ids(root, &runtime_owned, &mut ids);
    }
    Ok(ids)
}

fn collect_editable_ids(
    component: &ComponentNode,
    runtime_owned: &BTreeSet<String>,
    ids: &mut Vec<String>,
) {
    let Some(component) = component.as_object() else {
        return;
    };
    if is_inline_text_component(component)
        && let Some(id) = component.id()
        && !runtime_owned.contains(id)
    {
        ids.push(id.to_string());
    }
    for child in component.children() {
        collect_editable_ids(child, runtime_owned, ids);
    }
}

fn runtime_owned_component_ids(document: &ProjectDocument) -> BTreeSet<String> {
    let mut direct = BTreeSet::new();
    for collection in RUNTIME_COMPONENT_COLLECTIONS {
        let Some(entries) = document
            .project
            .extensions
            .get(collection)
            .and_then(Value::as_array)
        else {
            continue;
        };
        for entry in entries {
            if let Some(component_id) = entry
                .get("component_id")
                .or_else(|| entry.get("componentId"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                direct.insert(component_id.to_string());
            }
        }
    }

    let mut blocked = direct.clone();
    for page in &document.project.pages {
        if let Some(root) = page.component.as_ref() {
            collect_runtime_owned_subtree(root, false, &direct, &mut blocked);
        }
    }
    blocked
}

fn collect_runtime_owned_subtree(
    component: &ComponentNode,
    ancestor_blocked: bool,
    direct: &BTreeSet<String>,
    blocked: &mut BTreeSet<String>,
) {
    let Some(component) = component.as_object() else {
        return;
    };
    let id = component.id();
    let blocked_here = ancestor_blocked || id.is_some_and(|id| direct.contains(id));
    if blocked_here && let Some(id) = id {
        blocked.insert(id.to_string());
    }
    for child in component.children() {
        collect_runtime_owned_subtree(child, blocked_here, direct, blocked);
    }
}

fn selected_page_index(
    document: &ProjectDocument,
    selection: &PageSelection,
) -> Result<usize, InlineEditError> {
    let index = match selection {
        PageSelection::First => (!document.project.pages.is_empty()).then_some(0),
        PageSelection::Index(index) => document.project.pages.get(*index).map(|_| *index),
        PageSelection::Id(id) => document
            .project
            .pages
            .iter()
            .position(|page| page.id.as_deref() == Some(id.as_str())),
        PageSelection::Slug(_) => return Err(InlineEditError::UnsupportedSelection),
    };
    index.ok_or(InlineEditError::SelectedPageNotFound)
}

fn render_authenticated_inline_page(
    project_data: Value,
    selection: PageSelection,
    mut policy: RenderPolicy,
    context: Option<Value>,
) -> fly::FlyResult<(RenderedPage, usize, usize)> {
    policy.instrument_components = true;
    match context {
        Some(context) => {
            let PageBuilderRuntimeRenderResponse { result } =
                render_page_builder_runtime(PageBuilderRuntimeRenderRequest {
                    project_data,
                    selection,
                    policy,
                    context,
                })?;
            let diagnostic_count = result.diagnostics.len();
            let repeated_nodes = result.repeated_nodes;
            Ok((result.page, diagnostic_count, repeated_nodes))
        }
        None => {
            let PageBuilderRenderResponse { page } =
                render_page_builder_project(PageBuilderRenderRequest {
                    project_data,
                    selection,
                    policy,
                })?;
            Ok((page, 0, 0))
        }
    }
}

#[component]
pub fn PageBuilderAuthenticatedInlineStorefront(
    project_data: Value,
    grant: AuthenticatedInlineEditGrant,
    on_request: Callback<AuthenticatedInlineEditRequest>,
    on_error: Callback<String>,
    #[prop(optional)] selection: Option<PageSelection>,
    #[prop(optional)] policy: Option<RenderPolicy>,
    #[prop(optional)] context: Option<Value>,
    #[prop(optional)] class: Option<String>,
) -> impl IntoView {
    let selection = selection.unwrap_or(PageSelection::First);
    let policy = policy.unwrap_or_default();
    let class = class.unwrap_or_else(|| "rustok-page-builder-inline-storefront".to_string());
    let root_id = inline_root_id(&grant);
    let editable_ids = inline_editable_component_ids(&project_data, &selection).unwrap_or_default();
    let rendered = render_authenticated_inline_page(project_data, selection, policy, context);

    #[cfg(all(target_arch = "wasm32", feature = "hydrate"))]
    {
        let subscription =
            StoredValue::new_local(None::<fly_leptos::RealDomInlineEditSubscription>);
        let root_id = root_id.clone();
        let grant = grant.clone();
        let editable_ids = editable_ids.clone();
        let on_request = on_request.clone();
        let on_error = on_error.clone();
        Effect::new(move |_| {
            subscription.set_value(None);
            match fly_leptos::attach_real_dom_inline_editing(
                &root_id,
                editable_ids.clone(),
                grant.clone(),
                {
                    let on_request = on_request.clone();
                    move |request| on_request.run(request)
                },
                {
                    let on_error = on_error.clone();
                    move |error| on_error.run(error.to_string())
                },
            ) {
                Ok(value) => subscription.set_value(Some(value)),
                Err(error) => on_error.run(error.to_string()),
            }
        });
        on_cleanup(move || subscription.set_value(None));
    }

    match rendered {
        Ok((page, diagnostic_count, repeated_nodes)) => {
            let css = page.css;
            let html = page.html;
            let page_id = page.page_id.unwrap_or_default();
            view! {
                <section
                    id=root_id
                    class=class
                    data-rustok-page-builder-inline-storefront="true"
                    data-page-id=page_id
                    data-inline-revision=grant.revision_id().to_string()
                    data-inline-project-hash=grant.expected_project_hash().hex()
                    data-runtime-diagnostics=diagnostic_count
                    data-repeated-nodes=repeated_nodes
                >
                    <style data-fly-project-styles="true">{css}</style>
                    <div data-fly-page-body="true" inner_html=html></div>
                </section>
            }
            .into_any()
        }
        Err(error) => view! {
            <section
                id=root_id
                class=class
                data-rustok-page-builder-inline-storefront="true"
                data-render-error="true"
                role="alert"
            >
                <p>{error.to_string()}</p>
            </section>
        }
        .into_any(),
    }
}

fn inline_root_id(grant: &AuthenticatedInlineEditGrant) -> String {
    format!(
        "fly-inline-{}-{}",
        dom_id(grant.page_id()),
        grant.expected_project_hash().hex()
    )
}

fn dom_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> Value {
        json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "heading",
                        "type": "heading",
                        "content": "Hello"
                    }, {
                        "id": "dynamic",
                        "type": "text",
                        "content": "{{entry.name}}"
                    }, {
                        "id": "bound",
                        "type": "text",
                        "content": "Fallback"
                    }, {
                        "id": "interactive",
                        "type": "button",
                        "tagName": "button",
                        "content": "Go"
                    }, {
                        "id": "composite",
                        "type": "text",
                        "content": "Parent",
                        "components": [{ "id": "static-child", "type": "text", "content": "Child" }]
                    }, {
                        "id": "repeated",
                        "type": "wrapper",
                        "components": [{
                            "id": "repeated-child",
                            "type": "text",
                            "content": "Repeated child"
                        }]
                    }]
                }
            }],
            "flyRuntimeBindings": [{
                "id": "bound-content",
                "component_id": "bound",
                "path": "page.title",
                "target": "field",
                "name": "content"
            }],
            "flyRuntimeRepeaters": [{
                "id": "items",
                "component_id": "repeated",
                "path": "entries",
                "item_alias": "entry"
            }]
        })
    }

    fn grant(project_data: &Value) -> AuthenticatedInlineEditGrant {
        let document = GrapesJsCodec::decode_value(project_data.clone()).expect("document");
        AuthenticatedInlineEditGrant::new(
            "session-a",
            "home",
            "revision-a",
            document.hash(),
            "signed-proof",
            10_000,
        )
        .expect("grant")
    }

    #[test]
    fn only_noninteractive_static_leaf_text_outside_runtime_subtrees_is_editable() {
        let ids =
            inline_editable_component_ids(&project(), &PageSelection::First).expect("editable ids");
        assert_eq!(ids, vec!["heading", "static-child"]);
    }

    #[test]
    fn authorized_request_applies_one_canonical_fly_patch() {
        let project = project();
        let grant = grant(&project);
        let mut session = AuthenticatedInlineEditSession::new(
            project,
            PageSelection::First,
            grant.clone(),
            1_000,
        )
        .expect("session");
        let request = grant
            .bind_request(1_000, 1, "heading", "Updated")
            .expect("request");
        let result = session
            .apply_authorized(
                request,
                1_000,
                &|request: &AuthenticatedInlineEditRequest| {
                    (request.authorization_proof() == "signed-proof")
                        .then_some(())
                        .ok_or_else(|| "invalid proof".to_string())
                },
            )
            .expect("apply");
        assert_ne!(result.previous_hash, result.project_hash);
        assert_eq!(result.command_sequence, 1);
        assert_eq!(
            result.project_data["pages"][0]["component"]["components"][0]["content"].as_str(),
            Some("Updated")
        );
    }

    #[test]
    fn unchanged_focusout_does_not_consume_the_one_commit_grant() {
        let project = project();
        let grant = grant(&project);
        let mut session = AuthenticatedInlineEditSession::new(
            project,
            PageSelection::First,
            grant.clone(),
            1_000,
        )
        .expect("session");
        let unchanged = grant
            .bind_request(1_000, 1, "heading", "Hello")
            .expect("request");
        assert!(matches!(
            session.apply_authorized(unchanged, 1_000, &|_| Ok(())),
            Err(InlineEditError::NoContentChange(_))
        ));
        assert_eq!(session.last_sequence, 0);
        assert_eq!(
            session.current_project_hash(),
            grant.expected_project_hash()
        );
    }

    #[test]
    fn stale_replay_dynamic_bound_repeated_interactive_and_rejected_authorization_fail_closed() {
        let project = project();
        let grant = grant(&project);
        let mut session = AuthenticatedInlineEditSession::new(
            project,
            PageSelection::First,
            grant.clone(),
            1_000,
        )
        .expect("session");
        let rejected = grant
            .bind_request(1_000, 1, "heading", "Updated")
            .expect("request");
        assert!(matches!(
            session.apply_authorized(rejected, 1_000, &|_| Err("denied".to_string())),
            Err(InlineEditError::AuthorizationRejected(_))
        ));

        for component_id in ["dynamic", "bound", "repeated-child", "interactive"] {
            let request = grant
                .bind_request(1_000, 1, component_id, "Changed")
                .expect("request");
            assert!(matches!(
                session.apply_authorized(request, 1_000, &|_| Ok(())),
                Err(InlineEditError::ComponentNotInlineEditable(_))
            ));
        }

        let first = grant
            .bind_request(1_000, 1, "heading", "Updated")
            .expect("request");
        session
            .apply_authorized(first.clone(), 1_000, &|_| Ok(()))
            .expect("first apply");
        assert!(matches!(
            session.apply_authorized(first, 1_000, &|_| Ok(())),
            Err(InlineEditError::SequenceReplay { .. })
                | Err(InlineEditError::StaleProjectHash { .. })
        ));
    }

    #[test]
    fn inline_dom_identity_excludes_grant_session_and_authorization_proof() {
        let project = project();
        let grant = grant(&project);
        let root_id = inline_root_id(&grant);
        assert_eq!(
            root_id,
            format!("fly-inline-home-{}", grant.expected_project_hash().hex())
        );
        assert!(!root_id.contains(grant.session_id()));
        assert!(!root_id.contains("signed-proof"));
    }

    #[test]
    fn inline_renderer_instruments_components_without_exposing_authorization_proof() {
        let project = project();
        let output = render_authenticated_inline_page(
            project,
            PageSelection::First,
            RenderPolicy::default(),
            None,
        )
        .expect("render");
        assert!(output.0.html.contains("data-fly-component-id=\"heading\""));
        assert!(!output.0.html.contains("signed-proof"));
    }
}
