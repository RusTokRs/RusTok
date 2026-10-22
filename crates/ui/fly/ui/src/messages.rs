//! Resolved UI strings for Fly editor surfaces.
//!
//! `fly-ui` stays framework- and platform-neutral, so it owns the *shape* of the
//! editor's message set while hosts own the catalogs: a host resolves these
//! strings from its own locale/message system (for the current locale) and passes
//! the result to the adapter workspaces. [`EditorMessages::english`] is the
//! built-in fallback, mirroring how contribution descriptors carry host-resolved
//! `messages` maps next to neutral contracts.

use fly::{PageMetadata, PageSummary};
use serde::{Deserialize, Serialize};

/// One page-properties form submission from an adapter workspace: identity
/// (name/id) plus the SEO / Open Graph metadata of the page the form edited.
/// Hosts turn it into `fly::PageCommand::Patch` operations (or the matching
/// browser intents in SSR mode).
#[derive(Debug, Clone, PartialEq)]
pub struct PagePropertiesUpdate {
    pub summary: PageSummary,
    pub name: String,
    pub page_id: String,
    pub metadata: PageMetadata,
}

/// Every user-visible string the adapter editor surfaces render. Field order
/// follows the workspace layout: toolbar, panels, token editor, canvas.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorMessages {
    pub undo: String,
    pub redo: String,
    pub remove: String,
    pub save_token: String,
    pub token_name: String,
    pub token_value: String,
    pub pages: String,
    pub new_page_name: String,
    pub add_page: String,
    pub remove_page: String,
    pub untitled_page: String,
    pub contributions_summary: String,
    pub properties: String,
    pub styles: String,
    pub selected_component: String,
    pub no_selection: String,
    pub component_type: String,
    pub tag_name: String,
    pub content: String,
    pub attribute_name: String,
    pub attribute_value: String,
    pub attributes: String,
    pub clear: String,
    pub style_property: String,
    pub style_value: String,
    pub assets: String,
    pub asset_id: String,
    pub asset_url: String,
    pub add_asset: String,
    pub use_asset: String,
    pub tokens_empty: String,
    pub responsive_styles: String,
    pub interaction_states: String,
    pub breakpoint: String,
    pub interaction_state: String,
    pub preview_breakpoint: String,
    pub page_name_label: String,
    pub page_id_label: String,
    pub seo_title: String,
    pub seo_description: String,
    pub slug: String,
    pub canonical_url: String,
    pub og_title: String,
    pub og_description: String,
    pub og_image: String,
    pub no_index: String,
    pub apply: String,
    pub move_up: String,
    pub move_down: String,
    pub palette: String,
    pub layers: String,
    pub theme_tokens: String,
    pub workspace_title: String,
    pub toolbar: String,
    pub preview_failed: String,
    #[serde(default = "default_edit_label")]
    pub edit: String,
    #[serde(default = "default_preview_label")]
    pub preview: String,
}

fn default_edit_label() -> String {
    "Edit".to_string()
}

fn default_preview_label() -> String {
    "Preview".to_string()
}

impl EditorMessages {
    /// The built-in English strings, identical to the adapters' historical
    /// hardcoded labels.
    pub fn english() -> Self {
        Self {
            undo: "Undo".to_string(),
            redo: "Redo".to_string(),
            remove: "Remove".to_string(),
            save_token: "Save token".to_string(),
            token_name: "Token name".to_string(),
            token_value: "Token value".to_string(),
            pages: "Pages".to_string(),
            new_page_name: "New page name".to_string(),
            add_page: "Add page".to_string(),
            remove_page: "Remove page".to_string(),
            untitled_page: "Untitled page".to_string(),
            contributions_summary:
                "Provider contributions: {registered} registered, {skipped} skipped".to_string(),
            properties: "Properties".to_string(),
            styles: "Styles".to_string(),
            selected_component: "Selected component".to_string(),
            no_selection: "None".to_string(),
            component_type: "Type".to_string(),
            tag_name: "Tag name".to_string(),
            content: "Content".to_string(),
            attribute_name: "Attribute name".to_string(),
            attribute_value: "Attribute value or JSON".to_string(),
            attributes: "Attributes".to_string(),
            clear: "Clear".to_string(),
            style_property: "Style property".to_string(),
            style_value: "Style value".to_string(),
            assets: "Assets".to_string(),
            asset_id: "Asset id".to_string(),
            asset_url: "Asset URL".to_string(),
            add_asset: "Add".to_string(),
            use_asset: "Use".to_string(),
            tokens_empty: "No design tokens yet. Add one, then reference it as var(--name)."
                .to_string(),
            responsive_styles: "Responsive styles".to_string(),
            interaction_states: "Interaction states".to_string(),
            breakpoint: "Breakpoint".to_string(),
            interaction_state: "State".to_string(),
            preview_breakpoint: "Preview breakpoint".to_string(),
            page_name_label: "Page name".to_string(),
            page_id_label: "Page id".to_string(),
            seo_title: "SEO title".to_string(),
            seo_description: "SEO description".to_string(),
            slug: "Slug".to_string(),
            canonical_url: "Canonical URL".to_string(),
            og_title: "Open Graph title".to_string(),
            og_description: "Open Graph description".to_string(),
            og_image: "Open Graph image".to_string(),
            no_index: "Prevent search indexing".to_string(),
            apply: "Apply".to_string(),
            move_up: "Move up".to_string(),
            move_down: "Move down".to_string(),
            palette: "Block palette".to_string(),
            layers: "Component layers".to_string(),
            theme_tokens: "Design tokens".to_string(),
            workspace_title: "Fly full editor".to_string(),
            toolbar: "Editor toolbar".to_string(),
            preview_failed: "Preview failed".to_string(),
            edit: default_edit_label(),
            preview: default_preview_label(),
        }
    }
}

impl Default for EditorMessages {
    fn default() -> Self {
        Self::english()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_the_default_message_set() {
        let messages = EditorMessages::default();
        assert_eq!(messages, EditorMessages::english());
        assert_eq!(messages.undo, "Undo");
        assert_eq!(messages.theme_tokens, "Design tokens");
        assert_eq!(messages.pages, "Pages");
        assert_eq!(messages.add_page, "Add page");
        assert_eq!(messages.slug, "Slug");
        assert!(messages.contributions_summary.contains("{registered}"));
        assert_eq!(messages.workspace_title, "Fly full editor");
        assert_eq!(messages.edit, "Edit");
        assert_eq!(messages.preview, "Preview");
    }

    #[test]
    fn message_set_round_trips_through_serde() {
        let messages = EditorMessages::english();
        let json = serde_json::to_string(&messages).expect("serialize messages");
        let restored: EditorMessages = serde_json::from_str(&json).expect("deserialize messages");
        assert_eq!(restored, messages);
    }
}
