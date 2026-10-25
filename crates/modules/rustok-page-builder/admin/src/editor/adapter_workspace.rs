//! Adapter workspace surface: mounts the shared `fly-leptos` editor workspace on
//! top of the admin runtime (audit P0-1, admin migration first slice).
//!
//! The workspace runs in SSR intent mode: every user action arrives here as a
//! typed [`BrowserIntentEnvelope`] and is dispatched into the admin controller
//! through `dispatch_browser_intent`, so the controller (and its draft session)
//! stays the single source of truth — the workspace never mutates a document
//! copy. It renders the controller's current document snapshot; after an intent
//! round-trip the host re-renders with the fresh snapshot, exactly like the SSR
//! panels. Panel strings are resolved from this module's own message catalog
//! into the neutral [`EditorMessages`] the adapter accepts.

use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly_browser::BrowserIntentEnvelope;
use fly_leptos::FlyEditorWorkspace;
use fly_ui::{ContributionAssemblyResult, EditorMessages};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use std::sync::Arc;

#[component]
pub fn AdapterWorkspacePanel(
    runtime: AdminEditorRuntime,
    #[prop(optional_no_strip)] contribution_assembly: Option<Arc<ContributionAssemblyResult>>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let messages = adapter_editor_messages(locale.as_deref());
    let document = runtime
        .controller
        .with(|controller| controller.editor().document().clone());
    let instance_id = runtime
        .controller
        .with(|controller| controller.page_id().to_string());
    let initial_page = runtime
        .controller
        .with(|controller| controller.active_page_index());
    let assembly = contribution_assembly.as_deref().cloned();
    let capabilities = runtime
        .controller
        .with(|controller| controller.ui().state.capabilities);
    let intent_runtime = runtime.clone();
    let error_runtime = runtime.clone();
    let on_error = Callback::new(move |error: String| {
        error_runtime.fail(error);
    });
    let on_intent = Callback::new(move |envelope: BrowserIntentEnvelope| {
        let result = intent_runtime
            .controller
            .try_update(|controller| crate::dispatch_browser_intent(controller, envelope));
        match result {
            Some(Ok(_)) => intent_runtime.last_error.set(None),
            Some(Err(error)) => intent_runtime.fail(error.to_string()),
            None => {
                intent_runtime.fail("Page Builder controller is no longer available".to_string())
            }
        }
    });

    view! {
        <FlyEditorWorkspace
            document=document
            contribution_assembly=assembly
            instance_id=instance_id
            on_intent=on_intent
            on_error=on_error
            messages=messages
            initial_page=initial_page
            capabilities=capabilities
        />
    }
}

/// Builds the adapter's neutral message set from this module's catalog, so the
/// workspace renders with the same localized strings as the SSR panels.
pub fn adapter_editor_messages(locale: Option<&str>) -> EditorMessages {
    EditorMessages {
        undo: t(locale, "page_builder.action.undo", "Undo"),
        redo: t(locale, "page_builder.action.redo", "Redo"),
        remove: t(locale, "page_builder.action.remove", "Remove"),
        save_token: t(locale, "page_builder.action.saveToken", "Save token"),
        token_name: t(locale, "page_builder.field.tokenName", "Token name"),
        token_value: t(locale, "page_builder.field.tokenValue", "Token value"),
        properties: t(locale, "page_builder.panel.properties", "Properties"),
        styles: t(locale, "page_builder.panel.styles", "Styles"),
        selected_component: t(
            locale,
            "page_builder.field.selectedComponent",
            "Selected component",
        ),
        no_selection: t(locale, "page_builder.field.none", "None"),
        component_type: t(locale, "page_builder.field.type", "Type"),
        tag_name: t(locale, "page_builder.field.tagName", "Tag name"),
        content: t(locale, "page_builder.field.content", "Content"),
        attribute_name: t(locale, "page_builder.field.attributeName", "Attribute name"),
        attribute_value: t(
            locale,
            "page_builder.field.attributeValue",
            "Attribute value or JSON",
        ),
        attributes: t(locale, "page_builder.field.attributes", "Attributes"),
        clear: t(locale, "page_builder.action.clear", "Clear"),
        style_property: t(locale, "page_builder.field.styleProperty", "Style property"),
        style_value: t(locale, "page_builder.field.styleValue", "Style value"),
        tokens_empty: t(
            locale,
            "page_builder.themeTokens.empty",
            "No design tokens yet. Add one, then reference it as var(--name).",
        ),
        responsive_styles: t(
            locale,
            "page_builder.panel.responsiveStyles",
            "Responsive styles",
        ),
        interaction_states: t(
            locale,
            "page_builder.panel.stateStyles",
            "Interaction states",
        ),
        breakpoint: t(locale, "page_builder.field.breakpoint", "Breakpoint"),
        interaction_state: t(locale, "page_builder.field.state", "State"),
        preview_breakpoint: t(
            locale,
            "page_builder.action.previewBreakpoint",
            "Preview breakpoint",
        ),
        assets: t(locale, "page_builder.panel.assets", "Assets"),
        asset_id: t(locale, "page_builder.field.assetId", "Asset id"),
        asset_url: t(locale, "page_builder.field.assetUrl", "Asset URL"),
        add_asset: t(locale, "page_builder.action.add", "Add"),
        use_asset: t(locale, "page_builder.action.select", "Use"),
        contributions_summary: t(
            locale,
            "page_builder.panel.contributionsSummary",
            "Provider contributions: {registered} registered, {skipped} skipped",
        ),
        page_name_label: t(locale, "page_builder.field.pageName", "Page name"),
        page_id_label: t(locale, "page_builder.field.pageId", "Page id"),
        seo_title: t(locale, "page_builder.field.seoTitle", "SEO title"),
        seo_description: t(
            locale,
            "page_builder.field.seoDescription",
            "SEO description",
        ),
        slug: t(locale, "page_builder.field.slug", "Slug"),
        canonical_url: t(locale, "page_builder.field.canonicalUrl", "Canonical URL"),
        og_title: t(
            locale,
            "page_builder.field.openGraphTitle",
            "Open Graph title",
        ),
        og_description: t(
            locale,
            "page_builder.field.openGraphDescription",
            "Open Graph description",
        ),
        og_image: t(
            locale,
            "page_builder.field.openGraphImage",
            "Open Graph image",
        ),
        no_index: t(
            locale,
            "page_builder.field.noIndex",
            "Prevent search indexing",
        ),
        apply: t(locale, "page_builder.action.apply", "Apply"),
        move_up: t(locale, "page_builder.action.moveUp", "Move up"),
        move_down: t(locale, "page_builder.action.moveDown", "Move down"),
        pages: t(locale, "page_builder.panel.pages", "Pages"),
        new_page_name: t(
            locale,
            "page_builder.ssrInspector.newPageNamePlaceholder",
            "New page name",
        ),
        add_page: t(locale, "page_builder.action.addPage", "Add page"),
        remove_page: t(
            locale,
            "page_builder.ssrInspector.removePage",
            "Remove page",
        ),
        untitled_page: t(locale, "page_builder.page.untitled", "Untitled page"),
        palette: t(locale, "page_builder.panel.palette", "Blocks"),
        layers: t(locale, "page_builder.panel.layers", "Layers"),
        theme_tokens: t(locale, "page_builder.panel.themeTokens", "Design tokens"),
        workspace_title: t(locale, "page_builder.workspace.title", "Page Builder"),
        toolbar: t(locale, "page_builder.workspace.toolbar", "Editor toolbar"),
        preview_failed: t(
            locale,
            "page_builder.workspace.previewFailed",
            "Preview failed",
        ),
        edit: t(locale, "page_builder.action.edit", "Edit"),
        preview: t(locale, "page_builder.action.preview", "Preview"),
    }
}
