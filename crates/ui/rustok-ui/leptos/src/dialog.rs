/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use leptos::children::{Children, ChildrenFn};
use leptos::html;
use leptos::portal::Portal;
use leptos::prelude::*;
use rustok_ui::{
    dialog_backdrop_classes, dialog_close_classes, dialog_content_classes,
    dialog_description_classes, dialog_footer_classes, dialog_header_classes, dialog_title_classes,
    merge_classes,
};

#[cfg(target_arch = "wasm32")]
thread_local! {
    static MODAL_SCROLL_LOCKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PREVIOUS_BODY_OVERFLOW: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn acquire_page_scroll_lock() {
    MODAL_SCROLL_LOCKS.with(|locks| {
        let current = locks.get();
        if current == 0 {
            if let Some(body) = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.body())
            {
                let previous = body
                    .style()
                    .get_property_value("overflow")
                    .unwrap_or_default();
                PREVIOUS_BODY_OVERFLOW.with(|value| *value.borrow_mut() = Some(previous));
                let _ = body.style().set_property("overflow", "hidden");
            }
        }
        locks.set(current.saturating_add(1));
    });
}

#[cfg(target_arch = "wasm32")]
fn release_page_scroll_lock() {
    MODAL_SCROLL_LOCKS.with(|locks| {
        let current = locks.get();
        if current == 0 {
            return;
        }
        locks.set(current - 1);
        if current == 1 {
            if let Some(body) = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.body())
            {
                let previous = PREVIOUS_BODY_OVERFLOW.with(|value| value.borrow_mut().take());
                if let Some(previous) = previous {
                    let _ = body.style().set_property("overflow", &previous);
                }
            }
        }
    });
}

#[cfg(any(target_arch = "wasm32", test))]
fn dialog_tab_target_index(
    shift_pressed: bool,
    active_index: Option<usize>,
    focusable_count: usize,
) -> Option<usize> {
    if focusable_count == 0 {
        return None;
    }

    let Some(active_index) = active_index.filter(|index| *index < focusable_count) else {
        return Some(if shift_pressed {
            focusable_count - 1
        } else {
            0
        });
    };

    if shift_pressed {
        Some(if active_index == 0 {
            focusable_count - 1
        } else {
            active_index - 1
        })
    } else {
        Some(if active_index + 1 == focusable_count {
            0
        } else {
            active_index + 1
        })
    }
}

#[cfg(target_arch = "wasm32")]
fn is_visible_focus_target(element: &web_sys::HtmlElement) -> bool {
    use leptos::wasm_bindgen::JsCast;

    let element = element.unchecked_ref::<web_sys::Element>();
    if element
        .matches(":disabled, input[type='hidden']")
        .unwrap_or(false)
        || element.get_client_rects().length() == 0
        || element
            .closest("[hidden], [inert], [aria-hidden='true']")
            .is_ok_and(|ancestor| ancestor.is_some())
    {
        return false;
    }

    let Some(window) = web_sys::window() else {
        return true;
    };
    let Ok(Some(style)) = window.get_computed_style(element) else {
        return true;
    };
    let display = style.get_property_value("display").unwrap_or_default();
    let visibility = style.get_property_value("visibility").unwrap_or_default();
    display != "none" && visibility != "hidden" && visibility != "collapse"
}

#[cfg(target_arch = "wasm32")]
fn trap_dialog_tab_sequence(event: &leptos::ev::KeyboardEvent) {
    use leptos::wasm_bindgen::JsCast;

    const FOCUSABLE_SELECTOR: &str = concat!(
        "a[href], area[href], ",
        "input:not(:disabled):not([type='hidden']), ",
        "select:not(:disabled), textarea:not(:disabled), ",
        "button:not(:disabled), iframe, object, embed, ",
        "audio[controls], video[controls], ",
        "[contenteditable]:not([contenteditable='false']), [tabindex]",
    );

    let Some(target) = event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
    else {
        return;
    };
    let Ok(Some(dialog_element)) = target.closest("[role='dialog']") else {
        return;
    };
    let Ok(dialog) = dialog_element.dyn_into::<web_sys::HtmlElement>() else {
        return;
    };
    let Ok(nodes) = dialog
        .unchecked_ref::<web_sys::Element>()
        .query_selector_all(FOCUSABLE_SELECTOR)
    else {
        event.prevent_default();
        let _ = dialog.focus();
        return;
    };

    let mut focusables = Vec::new();
    for index in 0..nodes.length() {
        let Some(node) = nodes.item(index) else {
            continue;
        };
        let Ok(element) = node.dyn_into::<web_sys::HtmlElement>() else {
            continue;
        };
        if element.tab_index() < 0 || !is_visible_focus_target(&element) {
            continue;
        }
        focusables.push(element);
    }

    // Positive tabindex values precede the natural (zero) tab order.
    focusables.sort_by_key(|element| {
        let tab_index = element.tab_index();
        (tab_index == 0, tab_index)
    });

    let active = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.active_element())
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok());
    let active_index = active.as_ref().and_then(|active| {
        let active_node = active.unchecked_ref::<web_sys::Node>();
        focusables.iter().position(|focusable| {
            active_node.is_same_node(Some(focusable.unchecked_ref::<web_sys::Node>()))
        })
    });

    event.prevent_default();
    if let Some(next_index) =
        dialog_tab_target_index(event.shift_key(), active_index, focusables.len())
    {
        let _ = focusables[next_index].focus();
    } else {
        let _ = dialog.focus();
    }
}

#[derive(Clone, Copy)]
struct DialogContext {
    open: Signal<bool>,
    set_open: Callback<bool>,
    modal: bool,
    last_trigger: RwSignal<Option<NodeRef<html::Button>>>,
}

fn expect_dialog_context() -> DialogContext {
    use_context::<DialogContext>()
        .expect("Dialog parts must be rendered as descendants of <Dialog>")
}

#[cfg(test)]
mod tests {
    use super::dialog_tab_target_index;

    #[test]
    fn tab_sequence_wraps_and_enters_at_the_first_focusable() {
        assert_eq!(dialog_tab_target_index(false, None, 3), Some(0));
        assert_eq!(dialog_tab_target_index(false, Some(0), 3), Some(1));
        assert_eq!(dialog_tab_target_index(false, Some(2), 3), Some(0));
    }

    #[test]
    fn reverse_tab_sequence_wraps_and_enters_at_the_last_focusable() {
        assert_eq!(dialog_tab_target_index(true, None, 3), Some(2));
        assert_eq!(dialog_tab_target_index(true, Some(2), 3), Some(1));
        assert_eq!(dialog_tab_target_index(true, Some(0), 3), Some(2));
    }

    #[test]
    fn tab_sequence_handles_a_single_or_empty_focusable_set() {
        assert_eq!(dialog_tab_target_index(false, Some(0), 1), Some(0));
        assert_eq!(dialog_tab_target_index(true, Some(0), 1), Some(0));
        assert_eq!(dialog_tab_target_index(false, None, 0), None);
        assert_eq!(dialog_tab_target_index(true, None, 0), None);
    }
}

/// Dialog root that provides controlled or uncontrolled open state to its parts.
///
/// Pass `open` together with `on_open_change` for controlled state, or use
/// `default_open` for an uncontrolled dialog. `modal` defaults to `true`.
#[component]
pub fn Dialog(
    #[prop(optional, into)] open: MaybeProp<bool>,
    #[prop(default = false)] default_open: bool,
    #[prop(optional)] on_open_change: Option<Callback<bool>>,
    #[prop(default = true)] modal: bool,
    children: Children,
) -> impl IntoView {
    let internal_open = RwSignal::new(default_open);
    let open_prop = open;
    let open_signal =
        Signal::derive(move || open_prop.get().unwrap_or_else(|| internal_open.get()));
    let last_trigger = RwSignal::new(None::<NodeRef<html::Button>>);
    let set_open = Callback::new(move |next_open| {
        if open_prop.get().is_none() {
            internal_open.set(next_open);
        }
        if let Some(callback) = on_open_change {
            callback.run(next_open);
        }
    });
    provide_context(DialogContext {
        open: open_signal,
        set_open,
        modal,
        last_trigger,
    });

    // When a dialog is closed by its content, restore focus to the trigger that
    // opened it. Programmatic opens without a DialogTrigger remain host-owned.
    let was_open = RwSignal::new(open_signal.get_untracked());
    Effect::new(move |_| {
        let is_open = open_signal.get();
        if was_open.get_untracked() && !is_open {
            if let Some(trigger) = last_trigger
                .get_untracked()
                .and_then(|node_ref| node_ref.get())
            {
                let _ = trigger.focus();
            }
        }
        was_open.set(is_open);
    });

    #[cfg(target_arch = "wasm32")]
    {
        let scroll_locked = RwSignal::new(false);
        Effect::new(move |_| {
            let should_lock = modal && open_signal.get();
            match (should_lock, scroll_locked.get_untracked()) {
                (true, false) => {
                    acquire_page_scroll_lock();
                    scroll_locked.set(true);
                }
                (false, true) => {
                    release_page_scroll_lock();
                    scroll_locked.set(false);
                }
                _ => {}
            }
        });
        on_cleanup(move || {
            if scroll_locked.get_untracked() {
                release_page_scroll_lock();
            }
        });
    }

    children()
}

/// Button that toggles the containing dialog.
///
/// Set `aria_controls` to the corresponding [`DialogContent`] `id` to provide
/// the trigger/panel relationship; automatic Radix-style ID generation is not
/// yet provided.
#[component]
pub fn DialogTrigger(
    #[prop(default = false)] disabled: bool,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] aria_label: String,
    #[prop(optional, into)] aria_controls: String,
    #[prop(optional, into)] class: String,
    #[prop(default = "button")] r#type: &'static str,
    children: Children,
) -> impl IntoView {
    let context = expect_dialog_context();
    let id = (!id.is_empty()).then_some(id);
    let aria_label = (!aria_label.is_empty()).then_some(aria_label);
    let aria_controls = (!aria_controls.is_empty()).then_some(aria_controls);
    let trigger_ref = NodeRef::<html::Button>::new();
    view! {
        <button
            node_ref=trigger_ref
            id=id
            aria-label=aria_label
            type=r#type
            data-slot="dialog-trigger"
            data-state=move || if context.open.get() { "open" } else { "closed" }
            aria-haspopup="dialog"
            aria-controls=aria_controls
            aria-expanded=move || if context.open.get() { "true" } else { "false" }
            disabled=disabled
            class=class
            on:click=move |_| {
                if !disabled {
                    context.last_trigger.set(Some(trigger_ref));
                    context.set_open.run(!context.open.get());
                }
            }
        >
            {children()}
        </button>
    }
}

/// Renders dialog content into a Leptos portal in `document.body` while open.
#[component]
pub fn DialogPortal(children: ChildrenFn) -> impl IntoView {
    let context = expect_dialog_context();
    view! {
        {move || {
            let children = children.clone();
            context.open.get().then(move || view! {
                <Portal>
                    {children()}
                </Portal>
            })
        }}
    }
}

/// Button that closes the containing dialog.
#[component]
pub fn DialogClose(
    #[prop(default = false)] disabled: bool,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] aria_label: String,
    #[prop(optional, into)] class: String,
    #[prop(default = "button")] r#type: &'static str,
    children: Children,
) -> impl IntoView {
    let context = expect_dialog_context();
    let id = (!id.is_empty()).then_some(id);
    let aria_label = (!aria_label.is_empty()).then_some(aria_label);
    view! {
        <button
            type=r#type
            id=id
            aria-label=aria_label
            data-slot="dialog-close"
            data-state=move || if context.open.get() { "open" } else { "closed" }
            disabled=disabled
            class=class
            on:click=move |_| {
                if !disabled && context.open.get() {
                    context.set_open.run(false);
                }
            }
        >
            {children()}
        </button>
    }
}

/// Full-screen overlay used by modal dialogs. Clicking it requests closure.
#[component]
pub fn DialogOverlay(
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let context = expect_dialog_context();
    let id = (!id.is_empty()).then_some(id);
    let base_class = dialog_backdrop_classes(true);
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = merge_classes(&[base_class.as_str(), custom.unwrap_or("")]);

    view! {
        <div
            id=id
            data-slot="dialog-overlay"
            data-state=move || if context.open.get() { "open" } else { "closed" }
            class=full_class
            on:click=move |_| context.set_open.run(false)
        />
    }
}

/// Dialog panel rendered through a body portal, with overlay and close control.
///
/// For an accessible name, set `aria_label` or provide `aria_labelledby` that
/// references a [`DialogTitle`] `id`. `aria_describedby` can reference a
/// [`DialogDescription`] `id`. Modal content focuses on open and traps Tab and
/// Shift+Tab; Escape, the overlay, and [`DialogClose`] request closure. Focus is
/// restored to the last [`DialogTrigger`] when it is still mounted.
#[component]
pub fn DialogContent(
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] class: String,
    #[prop(default = -1)] tabindex: i32,
    #[prop(optional, into)] aria_label: String,
    #[prop(optional, into)] aria_labelledby: String,
    #[prop(optional, into)] aria_describedby: String,
    children: ChildrenFn,
) -> impl IntoView {
    let context = expect_dialog_context();
    let id = (!id.is_empty()).then_some(id);
    let custom = (!class.is_empty()).then_some(class.as_str());
    let content_class = dialog_content_classes(true, custom);
    let close_class = dialog_close_classes(None);
    let label_attr = (!aria_label.is_empty()).then_some(aria_label);
    let labelledby_attr = (!aria_labelledby.is_empty()).then_some(aria_labelledby);
    let describedby_attr = (!aria_describedby.is_empty()).then_some(aria_describedby);
    let dialog_ref = NodeRef::<html::Div>::new();

    Effect::new(move |_| {
        if context.open.get()
            && let Some(element) = dialog_ref.get()
        {
            let _ = element.focus();
        }
    });

    let on_keydown = move |event: leptos::ev::KeyboardEvent| {
        if event.key() == "Escape" {
            event.prevent_default();
            event.stop_propagation();
            context.set_open.run(false);
        }

        #[cfg(target_arch = "wasm32")]
        if context.modal
            && event.key() == "Tab"
            && !event.alt_key()
            && !event.ctrl_key()
            && !event.meta_key()
        {
            trap_dialog_tab_sequence(&event);
        }
    };

    view! {
        <DialogPortal>
            {context.modal.then(|| view! { <DialogOverlay /> })}
            <div
                node_ref=dialog_ref
                id=id.clone()
                role="dialog"
                data-slot="dialog-content"
                data-state=move || if context.open.get() { "open" } else { "closed" }
                aria-modal=if context.modal { Some("true") } else { None }
                aria-label=label_attr.clone()
                aria-labelledby=labelledby_attr.clone()
                aria-describedby=describedby_attr.clone()
                tabindex=tabindex
                class=content_class.clone()
                on:keydown=on_keydown
            >
                {children()}
                <DialogClose class=close_class.clone()>
                    <svg
                        aria-hidden="true"
                        class="pointer-events-none size-4 shrink-0"
                        fill="none"
                        stroke="currentColor"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        viewBox="0 0 24 24"
                    >
                        <path d="M18 6 6 18" />
                        <path d="m6 6 12 12" />
                    </svg>
                    <span class="sr-only">"Close"</span>
                </DialogClose>
            </div>
        </DialogPortal>
    }
}

/// Header slot for a dialog's title and description.
#[component]
pub fn DialogHeader(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <div data-slot="dialog-header" class=dialog_header_classes(custom)>
            {children()}
        </div>
    }
}

/// Dialog heading. Set `id` when the parent names itself through
/// `aria_labelledby`.
#[component]
pub fn DialogTitle(
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let id = (!id.is_empty()).then_some(id);
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <h2 id=id data-slot="dialog-title" class=dialog_title_classes(custom)>
            {children()}
        </h2>
    }
}

/// Supporting description. Give it an `id` to reference from `DialogContent`.
#[component]
pub fn DialogDescription(
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let id = (!id.is_empty()).then_some(id);
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <p id=id data-slot="dialog-description" class=dialog_description_classes(custom)>
            {children()}
        </p>
    }
}

/// Footer slot for dialog actions.
#[component]
pub fn DialogFooter(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <div data-slot="dialog-footer" class=dialog_footer_classes(custom)>
            {children()}
        </div>
    }
}
