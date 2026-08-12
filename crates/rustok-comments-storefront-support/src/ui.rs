use leptos::prelude::*;
use leptos_auth::hooks::use_is_authenticated;
use leptos_ui::{RichTextEditorFrame, RichTextFrameCopy};
use rustok_api::RichTextDocument;

use crate::core::is_richtext_blank;

#[derive(Clone, Debug)]
pub struct CommentComposerCopy {
    pub title: String,
    pub editor_label: String,
    pub hint: String,
    pub submit: String,
    pub submitting: String,
    pub success: String,
    pub sign_in_required: String,
    pub empty_error: String,
    pub richtext: RichTextFrameCopy,
}

#[component]
pub fn CommentComposer(
    content_locale: String,
    submit_action: Action<RichTextDocument, Result<(), String>>,
    copy: CommentComposerCopy,
) -> AnyView {
    let (document, set_document) = signal(RichTextDocument::empty());
    let validation_error = RwSignal::new(None::<String>);
    let is_authenticated = use_is_authenticated();
    let frame_copy = StoredValue::new(copy.richtext);
    let empty_error = StoredValue::new(copy.empty_error);
    let title = copy.title;
    let editor_label = StoredValue::new(copy.editor_label);
    let hint = StoredValue::new(copy.hint);
    let submit = StoredValue::new(copy.submit);
    let submitting = StoredValue::new(copy.submitting);
    let success = StoredValue::new(copy.success);
    let sign_in_required = StoredValue::new(copy.sign_in_required);
    let content_locale = StoredValue::new(content_locale);

    Effect::new(move |_| {
        if matches!(submit_action.value().get(), Some(Ok(()))) {
            set_document.set(RichTextDocument::empty());
            validation_error.set(None);
        }
    });

    view! {
        <section class="mt-6 rounded-2xl border border-border bg-card/50 p-5">
            <h4 class="text-base font-semibold text-foreground">{title}</h4>
            <Show
                when=move || is_authenticated.get()
                fallback=move || view! {
                    <p class="mt-3 rounded-xl border border-border bg-muted/40 p-4 text-sm text-muted-foreground">
                        {sign_in_required.get_value()}
                    </p>
                }
            >
                {move || {
                    let empty_error_value = empty_error.get_value();
                    let on_submit = move |event: leptos::ev::SubmitEvent| {
                        event.prevent_default();
                        let value = document.get_untracked();
                        if is_richtext_blank(&value) {
                            validation_error.set(Some(empty_error_value.clone()));
                            return;
                        }
                        validation_error.set(None);
                        submit_action.clear();
                        submit_action.dispatch(value);
                    };

                    view! {
                    <form class="mt-4 space-y-3" on:submit=on_submit>
                        <RichTextEditorFrame
                            document=document
                            set_document=set_document
                            content_locale=Signal::derive({
                                move || content_locale.get_value()
                            })
                            label=editor_label.get_value()
                            profile="comment".to_string()
                            copy=frame_copy.get_value()
                            disabled=Signal::derive(move || submit_action.pending().get())
                        />
                        <p class="text-xs text-muted-foreground">{hint.get_value()}</p>
                        {move || validation_error.get().map(|message| view! {
                            <p class="text-sm text-destructive" role="alert">{message}</p>
                        })}
                        {move || submit_action.value().get().and_then(Result::err).map(|message| view! {
                            <p class="text-sm text-destructive" role="alert">{message}</p>
                        })}
                        {move || matches!(submit_action.value().get(), Some(Ok(()))).then(|| view! {
                            <p class="text-sm text-emerald-700 dark:text-emerald-300" role="status">
                                {success.get_value()}
                            </p>
                        })}
                        <button
                            type="submit"
                            class="rounded-full bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground transition hover:opacity-95 disabled:cursor-not-allowed disabled:opacity-50"
                            disabled=move || submit_action.pending().get()
                        >
                            {move || if submit_action.pending().get() {
                                submitting.get_value()
                            } else {
                                submit.get_value()
                            }}
                        </button>
                    </form>
                    }
                }}
            </Show>
        </section>
    }
    .into_any()
}
