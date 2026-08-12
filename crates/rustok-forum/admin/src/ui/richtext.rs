use leptos::prelude::*;
use leptos_ui::{RichTextEditorFrame, localized_richtext_frame_copy};
use rustok_api::RichTextDocument;

use crate::i18n::t;

#[component]
pub fn ForumRichTextEditor(
    document: ReadSignal<RichTextDocument>,
    set_document: WriteSignal<RichTextDocument>,
    content_locale: ReadSignal<String>,
    label: String,
    #[prop(default = Signal::derive(|| false))] disabled: Signal<bool>,
) -> impl IntoView {
    let route_context = use_context::<rustok_ui_core::UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale;
    let copy = localized_richtext_frame_copy(|key, fallback| {
        t(ui_locale.as_deref(), key, fallback)
    });

    view! {
        <RichTextEditorFrame
            document=document
            set_document=set_document
            content_locale=Signal::from(content_locale)
            label=label
            profile="discussion".to_string()
            copy=copy
            disabled=disabled
        />
    }
}
