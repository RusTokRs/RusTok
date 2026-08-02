use leptos::prelude::*;
use leptos_ui::{RichTextEditorFrame, localized_richtext_frame_copy};
use rustok_api::RichTextDocument;

use crate::i18n::t;

#[component]
pub fn ForumRichTextEditor(
    document: ReadSignal<RichTextDocument>,
    set_document: WriteSignal<RichTextDocument>,
    label: String,
) -> impl IntoView {
    let route_context = use_context::<rustok_ui_core::UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.unwrap_or_else(|| "en".to_string());
    let copy =
        localized_richtext_frame_copy(|key, fallback| t(Some(locale.as_str()), key, fallback));

    view! {
        <RichTextEditorFrame
            document=document
            set_document=set_document
            label=label
            profile="discussion".to_string()
            copy=copy
        />
    }
}
