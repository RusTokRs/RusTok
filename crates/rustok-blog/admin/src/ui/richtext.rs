use leptos::html;
use leptos::prelude::*;
use rustok_api::RichTextDocument;

use crate::i18n::t;

#[component]
pub fn BlogRichTextEditor(
    document: ReadSignal<RichTextDocument>,
    set_document: WriteSignal<RichTextDocument>,
    label: String,
) -> impl IntoView {
    let route_context = use_context::<rustok_ui_core::UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.unwrap_or_else(|| "en".to_string());
    let iframe_ref = NodeRef::<html::Iframe>::new();
    let messages = serde_json::json!({
        "bold": t(Some(locale.as_str()), "richText.bold", "Bold"),
        "italic": t(Some(locale.as_str()), "richText.italic", "Italic"),
        "strike": t(Some(locale.as_str()), "richText.strike", "Strike"),
        "code": t(Some(locale.as_str()), "richText.code", "Code"),
        "heading": t(Some(locale.as_str()), "richText.heading", "Heading"),
        "bullet_list": t(Some(locale.as_str()), "richText.bullet_list", "Bullet list"),
        "ordered_list": t(Some(locale.as_str()), "richText.ordered_list", "Ordered list"),
        "blockquote": t(Some(locale.as_str()), "richText.blockquote", "Blockquote"),
        "code_block": t(Some(locale.as_str()), "richText.code_block", "Code block"),
        "horizontal_rule": t(Some(locale.as_str()), "richText.horizontal_rule", "Horizontal rule"),
        "link": t(Some(locale.as_str()), "richText.link", "Link"),
        "link_url": t(Some(locale.as_str()), "richText.link_url", "Link URL"),
        "apply_link": t(Some(locale.as_str()), "richText.apply_link", "Apply link"),
        "remove_link": t(Some(locale.as_str()), "richText.remove_link", "Remove link"),
        "clear_formatting": t(Some(locale.as_str()), "richText.clear_formatting", "Clear formatting"),
        "undo": t(Some(locale.as_str()), "richText.undo", "Undo"),
        "redo": t(Some(locale.as_str()), "richText.redo", "Redo"),
        "editor": t(Some(locale.as_str()), "richText.editor", "Rich text editor")
    });

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::{JsValue, closure::Closure};
        use web_sys::HtmlIFrameElement;

        #[wasm_bindgen::wasm_bindgen]
        extern "C" {
            #[wasm_bindgen::wasm_bindgen(
                js_namespace = RustokRichText,
                js_name = mountLeptosRichTextFrame
            )]
            fn mount_richtext_frame(
                iframe: &HtmlIFrameElement,
                frame_url: &str,
                profile: &str,
                document_json: &str,
                messages_json: &str,
                editable: bool,
                on_document_change: &Closure<dyn FnMut(String)>,
                on_error: &Closure<dyn FnMut(String, String)>,
            ) -> JsValue;

            #[wasm_bindgen::wasm_bindgen(
                js_namespace = RustokRichText,
                js_name = disposeLeptosRichTextFrame
            )]
            fn dispose_richtext_frame(handle: &JsValue);
        }

        let initial_document = document.get_untracked();
        let messages_json =
            serde_json::to_string(&messages).expect("richtext messages must serialize");
        let iframe_ref = iframe_ref;
        on_mount(move || {
            let Some(iframe) = iframe_ref.get() else {
                return;
            };
            let on_document_change =
                Closure::<dyn FnMut(String)>::new(move |document_json| {
                    if let Ok(document) = serde_json::from_str::<RichTextDocument>(&document_json) {
                        set_document.set(document);
                    }
                });
            let on_error = Closure::<dyn FnMut(String, String)>::new(move |_code, _message| {});
            let document_json =
                serde_json::to_string(&initial_document).expect("document must serialize");
            let handle = mount_richtext_frame(
                &iframe,
                "/richtext/frame",
                "article",
                &document_json,
                &messages_json,
                true,
                &on_document_change,
                &on_error,
            );
            on_cleanup(move || {
                dispose_richtext_frame(&handle);
                drop(on_document_change);
                drop(on_error);
            });
        });
    }

    view! {
        <div class="space-y-2">
            <label class="text-sm font-medium">{label.clone()}</label>
            <iframe
                node_ref=iframe_ref
                title=label
                sandbox="allow-scripts"
                referrerpolicy="no-referrer"
                style="width:100%;min-height:18rem;border:0"
            ></iframe>
        </div>
    }
}
