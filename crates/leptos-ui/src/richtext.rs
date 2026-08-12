use leptos::html;
use leptos::prelude::*;
use rustok_api::{RichTextDocument, RichTextView, normalize_locale_tag};

#[derive(Clone, Debug)]
pub struct RichTextFrameCopy {
    messages: serde_json::Value,
    serialization_error: String,
    invalid_payload_error: String,
    frame_error: String,
}

pub fn localized_richtext_frame_copy(
    mut translate: impl FnMut(&str, &str) -> String,
) -> RichTextFrameCopy {
    RichTextFrameCopy {
        messages: serde_json::json!({
            "bold": translate("richText.bold", "Bold"),
            "italic": translate("richText.italic", "Italic"),
            "strike": translate("richText.strike", "Strike"),
            "code": translate("richText.code", "Code"),
            "heading": translate("richText.heading", "Heading"),
            "bullet_list": translate("richText.bullet_list", "Bullet list"),
            "ordered_list": translate("richText.ordered_list", "Ordered list"),
            "blockquote": translate("richText.blockquote", "Blockquote"),
            "code_block": translate("richText.code_block", "Code block"),
            "horizontal_rule": translate("richText.horizontal_rule", "Horizontal rule"),
            "link": translate("richText.link", "Link"),
            "link_url": translate("richText.link_url", "Link URL"),
            "apply_link": translate("richText.apply_link", "Apply link"),
            "remove_link": translate("richText.remove_link", "Remove link"),
            "clear_formatting": translate("richText.clear_formatting", "Clear formatting"),
            "undo": translate("richText.undo", "Undo"),
            "redo": translate("richText.redo", "Redo"),
            "editor": translate("richText.editor", "Rich text editor")
        }),
        serialization_error: translate(
            "richText.error.serialize",
            "The richtext document could not be prepared.",
        ),
        invalid_payload_error: translate(
            "richText.error.invalidPayload",
            "The editor returned an invalid richtext document.",
        ),
        frame_error: translate(
            "richText.error.frameUnavailable",
            "The richtext editor is unavailable.",
        ),
    }
}

/// Renders only the server-derived richtext projection. This boundary accepts
/// `RichTextView` rather than arbitrary HTML so callers do not bypass the
/// canonical renderer with author-provided markup.
#[component]
pub fn RichTextHtml(
    view: RichTextView,
    content_locale: String,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let content_locale = normalize_locale_tag(&content_locale).unwrap_or_else(|| "und".to_string());

    view! {
        <div
            class=class
            data-richtext=""
            lang=content_locale
            dir="auto"
            inner_html=view.html
        ></div>
    }
}

#[component]
pub fn RichTextEditorFrame(
    document: ReadSignal<RichTextDocument>,
    set_document: WriteSignal<RichTextDocument>,
    content_locale: Signal<String>,
    label: String,
    profile: String,
    copy: RichTextFrameCopy,
    #[prop(default = Signal::derive(|| true))] spellcheck: Signal<bool>,
    #[prop(default = Signal::derive(|| false))] disabled: Signal<bool>,
) -> impl IntoView {
    let iframe_ref = NodeRef::<html::Iframe>::new();
    let editor_error = RwSignal::new(None::<String>);

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::prelude::{Closure, JsValue, wasm_bindgen};
        use web_sys::HtmlIFrameElement;

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
                content_locale: &str,
                spellcheck: bool,
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
                js_name = setLeptosRichTextAuthoringContext
            )]
            fn set_richtext_authoring_context(
                handle: &JsValue,
                content_locale: &str,
                spellcheck: bool,
            );

            #[wasm_bindgen(
                js_namespace = RustokRichText,
                js_name = setLeptosRichTextEditable
            )]
            fn set_richtext_editable(handle: &JsValue, editable: bool);

            #[wasm_bindgen(
                js_namespace = RustokRichText,
                js_name = disposeLeptosRichTextFrame
            )]
            fn dispose_richtext_frame(handle: &JsValue);
        }

        let messages_json =
            serde_json::to_string(&copy.messages).expect("richtext messages must serialize");
        let editor_handle = StoredValue::new_local(None::<JsValue>);
        let callback_handles = StoredValue::new_local(
            None::<(
                Closure<dyn FnMut(JsValue)>,
                Closure<dyn FnMut(JsValue, JsValue)>,
            )>,
        );
        let controlled_serialization_error = copy.serialization_error.clone();

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

        Effect::new(move |_| {
            let content_locale = content_locale.get();
            let spellcheck = spellcheck.get();
            let Some(handle) = editor_handle.get_value() else {
                return;
            };
            set_richtext_authoring_context(&handle, content_locale.as_str(), spellcheck);
        });

        Effect::new(move |_| {
            let editable = !disabled.get();
            let Some(handle) = editor_handle.get_value() else {
                return;
            };
            set_richtext_editable(&handle, editable);
        });

        let iframe_ref = iframe_ref;
        Effect::new(move |_| {
            if editor_handle.get_value().is_some() {
                return;
            }
            let Some(iframe) = iframe_ref.get() else {
                return;
            };
            let invalid_payload_error = copy.invalid_payload_error.clone();
            let frame_error = copy.frame_error.clone();
            let serialization_error = copy.serialization_error.clone();
            let on_document_change =
                Closure::<dyn FnMut(JsValue)>::new(move |document_json: JsValue| {
                    let Some(document_json) = document_json.as_string() else {
                        editor_error.set(Some(invalid_payload_error.clone()));
                        return;
                    };
                    match serde_json::from_str::<RichTextDocument>(&document_json) {
                        Ok(document) => {
                            editor_error.set(None);
                            set_document.set(document);
                        }
                        Err(_) => editor_error.set(Some(invalid_payload_error.clone())),
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
            let document_json = match serde_json::to_string(&document.get_untracked()) {
                Ok(document_json) => document_json,
                Err(_) => {
                    editor_error.set(Some(serialization_error));
                    return;
                }
            };
            let mounted_handle = mount_richtext_frame(
                &iframe,
                "/richtext/frame",
                &profile,
                &document_json,
                &messages_json,
                content_locale.get_untracked().as_str(),
                spellcheck.get_untracked(),
                !disabled.get_untracked(),
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
    {
        let RichTextFrameCopy {
            messages,
            serialization_error,
            invalid_payload_error,
            frame_error,
        } = copy;
        let _ = (
            document,
            set_document,
            content_locale,
            profile,
            messages,
            serialization_error,
            invalid_payload_error,
            frame_error,
            spellcheck,
            disabled,
        );
    }

    view! {
        <div class="space-y-2">
            <label class="text-sm font-medium">{label.clone()}</label>
            <iframe
                node_ref=iframe_ref
                title=label
                lang=move || content_locale.get()
                aria-disabled=move || disabled.get()
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
