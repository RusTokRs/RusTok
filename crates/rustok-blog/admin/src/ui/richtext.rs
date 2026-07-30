use leptos::html;
use leptos::prelude::*;
use rustok_api::RichTextDocument;

#[component]
pub fn BlogRichTextEditor(
    document: ReadSignal<RichTextDocument>,
    set_document: WriteSignal<RichTextDocument>,
    label: String,
) -> impl IntoView {
    let route_context = use_context::<rustok_ui_core::UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.unwrap_or_else(|| "en".to_string());
    let iframe_ref = NodeRef::<html::Iframe>::new();
    let editor_error = RwSignal::new(None::<String>);

    #[cfg(target_arch = "wasm32")]
    {
        use crate::i18n::t;
        use wasm_bindgen::prelude::{Closure, JsValue, wasm_bindgen};
        use web_sys::HtmlIFrameElement;

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
        let serialization_error = t(
            Some(locale.as_str()),
            "richText.error.serialize",
            "The richtext document could not be prepared.",
        );
        let invalid_payload_error = t(
            Some(locale.as_str()),
            "richText.error.invalidPayload",
            "The editor returned an invalid richtext document.",
        );
        let frame_error = t(
            Some(locale.as_str()),
            "richText.error.frameUnavailable",
            "The richtext editor is unavailable.",
        );

        #[wasm_bindgen]
        unsafe extern "C" {
            #[wasm_bindgen(
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
                on_document_change: &Closure<dyn FnMut(JsValue)>,
                on_error: &Closure<dyn FnMut(JsValue, JsValue)>,
            ) -> JsValue;

            #[wasm_bindgen(
                js_namespace = RustokRichText,
                js_name = setLeptosRichTextDocument
            )]
            fn set_richtext_document(handle: &JsValue, document_json: &str);

            #[wasm_bindgen(
                js_namespace = RustokRichText,
                js_name = disposeLeptosRichTextFrame
            )]
            fn dispose_richtext_frame(handle: &JsValue);
        }

        let messages_json =
            serde_json::to_string(&messages).expect("richtext messages must serialize");
        let editor_handle = StoredValue::new_local(None::<JsValue>);
        let callback_handles = StoredValue::new_local(
            None::<(
                Closure<dyn FnMut(JsValue)>,
                Closure<dyn FnMut(JsValue, JsValue)>,
            )>,
        );
        let controlled_serialization_error = serialization_error.clone();

        Effect::new(move |_| {
            let document = document.get();
            let Some(handle) = editor_handle.get_value() else {
                return;
            };
            let Ok(document_json) = serde_json::to_string(&document) else {
                editor_error.set(Some(controlled_serialization_error.clone()));
                return;
            };
            set_richtext_document(&handle, &document_json);
        });

        let iframe_ref = iframe_ref;
        Effect::new(move |_| {
            if editor_handle.get_value().is_some() {
                return;
            }
            let Some(iframe) = iframe_ref.get() else {
                return;
            };
            let invalid_payload_error = invalid_payload_error.clone();
            let frame_error = frame_error.clone();
            let serialization_error = serialization_error.clone();
            let on_document_change =
                Closure::<dyn FnMut(JsValue)>::new(move |document_json: JsValue| {
                    let Some(document_json) = document_json.as_string() else {
                        editor_error.set(Some(invalid_payload_error.clone()));
                        return;
                    };
                    match serde_json::from_str::<RichTextDocument>(document_json.as_str()) {
                        Ok(document) => {
                            editor_error.set(None);
                            set_document.set(document);
                        }
                        Err(_) => {
                            editor_error.set(Some(invalid_payload_error.clone()));
                        }
                    }
                });
            let on_error = Closure::<dyn FnMut(JsValue, JsValue)>::new(
                move |code: JsValue, _message: JsValue| {
                    let code = code
                        .as_string()
                        .unwrap_or_else(|| "frame_error".to_string());
                    editor_error.set(Some(format!("{frame_error} ({code})")));
                },
            );
            let initial_document = document.get_untracked();
            let document_json = match serde_json::to_string(&initial_document) {
                Ok(document_json) => document_json,
                Err(_) => {
                    editor_error.set(Some(serialization_error.clone()));
                    return;
                }
            };
            let mounted_handle = mount_richtext_frame(
                &iframe,
                "/richtext/frame",
                "article",
                &document_json,
                &messages_json,
                true,
                &on_document_change,
                &on_error,
            );
            editor_handle.set_value(Some(mounted_handle.clone()));
            callback_handles.set_value(Some((on_document_change, on_error)));
            on_cleanup(move || {
                dispose_richtext_frame(&mounted_handle);
                editor_handle.set_value(None);
                callback_handles.set_value(None);
            });
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    let _ = (document, set_document, locale);

    view! {
        <div class="space-y-2">
            <label class="text-sm font-medium">{label.clone()}</label>
            <iframe
                node_ref=iframe_ref
                title=label
                sandbox="allow-scripts"
                referrerpolicy="no-referrer"
                class="h-72 w-full border-0"
            ></iframe>
            <Show when=move || editor_error.get().is_some()>
                <p class="text-sm text-destructive" role="alert">
                    {move || editor_error.get().unwrap_or_default()}
                </p>
            </Show>
        </div>
    }
}
